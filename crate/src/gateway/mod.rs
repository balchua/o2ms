use std::time::Duration;

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use oauth2_test_server::handlers::userinfo::userinfo;
use reqwest::Client;
use url::Url;

use crate::{
    app::state::WrapperState,
    config::model::{GatewayConfig, GatewayRouteConfig, HeaderValueFormat},
};

#[derive(Clone)]
pub struct GatewayRuntime {
    client: Client,
    max_body_bytes: usize,
    default_outbound_header_name: HeaderName,
    default_outbound_value_format: HeaderValueFormat,
    routes: Vec<CompiledRoute>,
}

#[derive(Clone)]
struct CompiledRoute {
    id: String,
    path_prefix: String,
    upstream_base_url: Url,
    auth_required: bool,
    outbound_header_name: Option<HeaderName>,
    outbound_value_format: Option<HeaderValueFormat>,
}

impl GatewayRuntime {
    pub fn from_config(config: &GatewayConfig) -> Result<Option<Self>, String> {
        if !config.enabled {
            return Ok(None);
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|error| format!("failed to build gateway HTTP client: {error}"))?;

        let default_outbound_header_name =
            HeaderName::from_bytes(config.auth.outbound_header_name.as_bytes()).map_err(|error| {
                format!(
                    "invalid gateway.auth.outbound_header_name '{}': {error}",
                    config.auth.outbound_header_name
                )
            })?;

        let mut routes = Vec::new();
        for route in &config.routes {
            if !route.enabled {
                continue;
            }
            routes.push(compile_route(route)?);
        }

        routes.sort_by(|left, right| right.path_prefix.len().cmp(&left.path_prefix.len()));

        Ok(Some(Self {
            client,
            max_body_bytes: config.max_body_bytes,
            default_outbound_header_name,
            default_outbound_value_format: config.auth.outbound_value_format.clone(),
            routes,
        }))
    }

    fn find_route(&self, path: &str) -> Option<&CompiledRoute> {
        self.routes.iter().find(|route| {
            path == route.path_prefix || path.strip_prefix(&route.path_prefix).is_some_and(|suffix| suffix.starts_with('/'))
        })
    }
}

fn compile_route(route: &GatewayRouteConfig) -> Result<CompiledRoute, String> {
    let upstream_base_url = Url::parse(&route.upstream_base_url)
        .map_err(|error| format!("invalid gateway route '{}' upstream URL: {error}", route.id))?;
    let outbound_header_name = route
        .outbound_header_name
        .as_deref()
        .map(|name| {
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                format!(
                    "invalid gateway route '{}' outbound_header_name '{}': {error}",
                    route.id, name
                )
            })
        })
        .transpose()?;

    Ok(CompiledRoute {
        id: route.id.clone(),
        path_prefix: route.path_prefix.clone(),
        upstream_base_url,
        auth_required: route.auth_required,
        outbound_header_name,
        outbound_value_format: route.outbound_value_format.clone(),
    })
}

pub async fn proxy_request(
    State(state): State<WrapperState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(runtime) = &state.gateway else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let path = uri.path();
    let Some(route) = runtime.find_route(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let bearer_token = extract_bearer_token(&headers);
    if route.auth_required {
        let Some(token) = bearer_token.as_deref() else {
            return gateway_error(StatusCode::UNAUTHORIZED, "missing_bearer_token");
        };
        if !token_is_valid(&state, token).await {
            return gateway_error(StatusCode::UNAUTHORIZED, "invalid_bearer_token");
        }
    }

    let upstream_url = match build_upstream_url(route, &uri) {
        Ok(url) => url,
        Err(error) => return gateway_error(StatusCode::BAD_REQUEST, &error),
    };

    let mut request_builder = runtime.client.request(method.clone(), upstream_url);
    for (header_name, header_value) in &headers {
        if should_forward_header(header_name) {
            request_builder = request_builder.header(header_name, header_value);
        }
    }
    if let Some(token) = bearer_token {
        let header_name = route
            .outbound_header_name
            .as_ref()
            .unwrap_or(&runtime.default_outbound_header_name);
        let value_format = route
            .outbound_value_format
            .as_ref()
            .unwrap_or(&runtime.default_outbound_value_format);
        let formatted_token = format_outbound_token(token.as_str(), value_format);
        request_builder = request_builder.header(header_name, formatted_token);
    }

    let upstream_response = match request_builder.body(body).send().await {
        Ok(response) => response,
        Err(error) if error.is_timeout() => {
            return gateway_error(StatusCode::GATEWAY_TIMEOUT, "upstream_timeout");
        }
        Err(_) => {
            return gateway_error(StatusCode::BAD_GATEWAY, "upstream_unavailable");
        }
    };

    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let response_body = match upstream_response.bytes().await {
        Ok(bytes) => bytes,
        Err(_) => {
            return gateway_error(StatusCode::BAD_GATEWAY, "upstream_response_read_failed");
        }
    };
    if response_body.len() > runtime.max_body_bytes {
        return gateway_error(StatusCode::BAD_GATEWAY, "upstream_response_too_large");
    }

    let mut response = Response::builder().status(status);
    for (name, value) in &headers {
        if should_forward_response_header(name) {
            response = response.header(name, value);
        }
    }

    match response.body(Body::from(response_body)) {
        Ok(result) => result,
        Err(_) => gateway_error(StatusCode::BAD_GATEWAY, "response_build_failed"),
    }
}

fn should_forward_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "accept" | "accept-language" | "content-type" | "x-request-id" | "x-correlation-id"
    )
}

fn should_forward_response_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "content-type" | "cache-control" | "etag" | "location" | "www-authenticate"
    )
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get(axum::http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
        Some(token.trim().to_string())
    } else {
        None
    }
}

fn format_outbound_token(token: &str, format: &HeaderValueFormat) -> String {
    if matches!(format, HeaderValueFormat::Raw) {
        token.to_string()
    } else {
        format!("{} {}", "Bearer", token)
    }
}

fn build_upstream_url(route: &CompiledRoute, uri: &Uri) -> Result<Url, String> {
    let mut upstream_url = route.upstream_base_url.clone();
    let suffix = uri.path().trim_start_matches(&route.path_prefix);

    let mut upstream_path = upstream_url.path().trim_end_matches('/').to_string();
    if suffix.is_empty() {
        if upstream_path.is_empty() {
            upstream_path.push('/');
        }
    } else {
        upstream_path.push_str(suffix);
    }

    upstream_url.set_path(&upstream_path);
    upstream_url.set_query(uri.query());
    Ok(upstream_url)
}

async fn token_is_valid(state: &WrapperState, token: &str) -> bool {
    let mut headers = HeaderMap::new();
    let Ok(value) = HeaderValue::from_str(&format!("{} {}", "Bearer", token)) else {
        return false;
    };
    headers.insert(axum::http::header::AUTHORIZATION, value);
    userinfo(headers, State(state.upstream.clone())).await.is_ok()
}

fn gateway_error(status: StatusCode, code: &str) -> Response {
    (status, [("content-type", "application/json")], format!(r#"{{"error":"{code}"}}"#))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{GatewayRuntime, build_upstream_url, extract_bearer_token, format_outbound_token};
    use crate::config::model::{GatewayConfig, GatewayRouteConfig, HeaderValueFormat};

    #[test]
    fn picks_longest_prefix_match() -> Result<(), Box<dyn std::error::Error>> {
        let config = GatewayConfig {
            enabled: true,
            routes: vec![
                GatewayRouteConfig {
                    id: "a".to_string(),
                    path_prefix: "/proxy".to_string(),
                    upstream_base_url: "http://127.0.0.1:9001".to_string(),
                    ..GatewayRouteConfig::default()
                },
                GatewayRouteConfig {
                    id: "b".to_string(),
                    path_prefix: "/proxy/users".to_string(),
                    upstream_base_url: "http://127.0.0.1:9002".to_string(),
                    ..GatewayRouteConfig::default()
                },
            ],
            ..GatewayConfig::default()
        };
        let runtime = GatewayRuntime::from_config(&config)?
            .ok_or("gateway runtime should be enabled for tests")?;

        let route = runtime
            .find_route("/proxy/users/me")
            .ok_or("expected matching route")?;
        assert_eq!(route.id, "b");
        Ok(())
    }

    #[test]
    fn builds_upstream_url_with_suffix_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let route = super::compile_route(&GatewayRouteConfig {
            id: "users".to_string(),
            path_prefix: "/proxy/users".to_string(),
            upstream_base_url: "http://127.0.0.1:9001/api".to_string(),
            ..GatewayRouteConfig::default()
        })?;
        let uri: axum::http::Uri = "/proxy/users/me?id=1".parse()?;
        let url = build_upstream_url(&route, &uri)?;
        assert_eq!(url.as_str(), "http://127.0.0.1:9001/api/me?id=1");
        Ok(())
    }

    #[test]
    fn extracts_bearer_token_from_authorization_header() {
        let mut headers = axum::http::HeaderMap::new();
        let value = format!("{} {}", "Bearer", "token-value");
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(value.as_str()).expect("valid bearer header"),
        );
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("token-value"));
    }

    #[test]
    fn formats_outbound_token_values() {
        assert_eq!(
            format_outbound_token("abc", &HeaderValueFormat::Bearer),
            format!("{} {}", "Bearer", "abc")
        );
        assert_eq!(format_outbound_token("abc", &HeaderValueFormat::Raw), "abc");
    }
}
