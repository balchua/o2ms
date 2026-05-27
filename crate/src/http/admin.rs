use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::Serialize;

use crate::{
    app::state::WrapperState,
    config::model::AppConfig,
    registry::clients::{find_enabled_client_by_id, seed_clients},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AdminClientView {
    client_id: String,
    client_name: Option<String>,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    scope: String,
    token_endpoint_auth_method: String,
    source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ResetResponse {
    clients_before: usize,
    codes_before: usize,
    tokens_before: usize,
    refresh_tokens_before: usize,
    clients_after: usize,
    reseeded_clients: usize,
}

pub fn routes(config: &AppConfig) -> Router<WrapperState> {
    let mut router = Router::new();

    if config.admin.list_clients_endpoint_enabled {
        router = router.route("/admin/clients", get(list_clients));
    }

    if config.admin.reset_endpoint_enabled {
        router = router.route("/admin/reset", post(reset_state));
    }

    if config.admin.config_endpoint_enabled {
        router = router.route("/admin/config", get(current_config));
    }

    router
}

#[must_use]
pub fn endpoints_enabled(config: &AppConfig) -> bool {
    config.admin.list_clients_endpoint_enabled
        || config.admin.reset_endpoint_enabled
        || config.admin.config_endpoint_enabled
}

#[must_use]
pub fn bind_host_allows_admin_endpoints(bind_host: &str) -> bool {
    matches!(bind_host, "127.0.0.1" | "::1" | "localhost")
}

async fn list_clients(State(state): State<WrapperState>) -> Json<Vec<AdminClientView>> {
    let mut clients = state.upstream.store.get_all_clients().await;
    clients.sort_by(|left, right| left.client_id.cmp(&right.client_id));

    Json(
        clients
            .into_iter()
            .map(|client| AdminClientView {
                source: if find_enabled_client_by_id(&state.config, &client.client_id).is_some() {
                    "preloaded".to_string()
                } else {
                    "runtime".to_string()
                },
                client_id: client.client_id,
                client_name: client.client_name,
                grant_types: client.grant_types,
                response_types: client.response_types,
                scope: client.scope,
                token_endpoint_auth_method: client.token_endpoint_auth_method,
            })
            .collect(),
    )
}

async fn reset_state(State(state): State<WrapperState>) -> Json<ResetResponse> {
    let clients_before = state.upstream.store.get_all_clients().await.len();
    let codes_before = state.upstream.store.get_all_codes().await.len();
    let tokens_before = state.upstream.store.get_all_tokens().await.len();
    let refresh_tokens_before = state.upstream.store.get_all_refresh_tokens().await.len();

    state.upstream.store.clear_all().await;
    let reseeded_clients = seed_clients(&state.config, &state.upstream)
        .await
        .map_or(0, |count| count);
    let clients_after = state.upstream.store.get_all_clients().await.len();

    Json(ResetResponse {
        clients_before,
        codes_before,
        tokens_before,
        refresh_tokens_before,
        clients_after,
        reseeded_clients,
    })
}

async fn current_config(State(state): State<WrapperState>) -> Json<AppConfig> {
    Json(state.config.clone())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::util::ServiceExt;

    use crate::{
        app::state::WrapperState,
        config::model::{AppConfig, ClientConfig},
        http::router::build_router,
        registry::clients::seed_clients,
        upstream::adapter::build_upstream_state,
    };

    use super::{bind_host_allows_admin_endpoints, endpoints_enabled};

    #[test]
    fn admin_endpoints_are_considered_disabled_by_default() {
        assert!(!endpoints_enabled(&AppConfig::default()));
    }

    #[test]
    fn admin_bind_host_must_be_loopback() {
        assert!(bind_host_allows_admin_endpoints("127.0.0.1"));
        assert!(bind_host_allows_admin_endpoints("localhost"));
        assert!(!bind_host_allows_admin_endpoints("0.0.0.0"));
    }

    #[tokio::test]
    async fn list_clients_route_returns_preloaded_clients(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.admin.list_clients_endpoint_enabled = true;
        config.clients = vec![ClientConfig {
            client_id: "demo-client".to_string(),
            client_name: "Demo Client".to_string(),
            ..ClientConfig::default()
        }];

        let upstream = build_upstream_state(&config, 8090);
        seed_clients(&config, &upstream).await?;
        let app = build_router(&config, WrapperState::new(config.clone(), upstream));
        let request = Request::builder()
            .method(Method::GET)
            .uri("/admin/clients")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }
}
