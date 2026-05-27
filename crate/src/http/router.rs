use axum::{
    http::{self, header, HeaderValue},
    routing::{get, post},
    Router,
};
use crate::{app::state::WrapperState, config::model::AppConfig};
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use super::{admin, health, oauth};

pub fn build_router(config: &AppConfig, state: WrapperState) -> Router {
    let mut router: Router<WrapperState> = Router::new();
    let admin_endpoints_enabled = admin::endpoints_enabled(config)
        && admin::bind_host_allows_admin_endpoints(config.server.bind_host.as_str());

    if config.server.health_endpoint_enabled {
        router = router.route("/health", get(health::health));
    }

    router = router
        .route(
            "/.well-known/openid-configuration",
            get(oauth::discovery),
        )
        .route(
            "/.well-known/jwks.json",
            get(oauth::jwks_doc),
        )
        .route("/authorize", get(oauth::authorize_flow))
        .route("/token", post(oauth::token_endpoint))
        .route("/device/code", post(oauth::device_code_proxy))
        .route("/device/token", post(oauth::device_token_proxy))
        .route("/introspect", post(oauth::introspect_proxy))
        .route("/revoke", post(oauth::revoke_proxy))
        .route("/userinfo", get(oauth::userinfo_proxy))
        .route("/error", get(oauth::error_page_proxy))
        .layer(build_cors_layer(config))
        .layer(TraceLayer::new_for_http());

    if admin_endpoints_enabled {
        router = router.merge(admin::routes(config));
    }

    if config.server.runtime_client_registration_enabled {
        router = router
            .route(
                "/register",
                post(oauth::register_client_proxy),
            )
            .route(
                "/register/{client_id}",
                get(oauth::get_client_proxy),
            );
    }

    router.with_state(state)
}

fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    let allowed_origins: Vec<HeaderValue> = config
        .server
        .cors_allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    let allowed_methods =
        AllowMethods::list([http::Method::GET, http::Method::POST, http::Method::OPTIONS]);
    let allowed_headers = AllowHeaders::list([
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::HeaderName::from_static("x-requested-with"),
    ]);

    let mut cors = CorsLayer::new()
        .allow_methods(allowed_methods)
        .allow_headers(allowed_headers)
        .max_age(std::time::Duration::from_hours(24));

    if allowed_origins.is_empty() {
        cors = cors.allow_origin(AllowOrigin::any());
    } else {
        cors = cors
            .allow_origin(AllowOrigin::list(allowed_origins))
            .allow_credentials(true);
    }

    cors
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    use crate::{
        app::state::WrapperState, config::model::AppConfig, upstream::adapter::build_upstream_state,
    };

    use super::build_router;

    #[tokio::test]
    async fn health_endpoint_is_available_on_wrapper_router(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let upstream = build_upstream_state(&AppConfig::default(), 8090);
        let app = build_router(
            &AppConfig::default(),
            WrapperState::new(AppConfig::default(), upstream),
        );
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn health_endpoint_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.server.health_endpoint_enabled = false;
        let upstream = build_upstream_state(&config, 8090);
        let app = build_router(&config, WrapperState::new(config.clone(), upstream));
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn runtime_registration_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.server.runtime_client_registration_enabled = false;
        let upstream = build_upstream_state(&config, 8090);
        let app = build_router(&config, WrapperState::new(config.clone(), upstream));
        let request = Request::builder()
            .method(http::Method::POST)
            .uri("/register")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn admin_endpoint_is_available_when_enabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.admin.list_clients_endpoint_enabled = true;
        let upstream = build_upstream_state(&config, 8090);
        let app = build_router(&config, WrapperState::new(config.clone(), upstream));
        let request = Request::builder()
            .uri("/admin/clients")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn admin_endpoints_are_not_mounted_for_non_loopback_bind_host(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.server.bind_host = "0.0.0.0".to_string();
        config.admin.list_clients_endpoint_enabled = true;
        let upstream = build_upstream_state(&config, 8090);
        let app = build_router(&config, WrapperState::new(config.clone(), upstream));
        let request = Request::builder()
            .uri("/admin/clients")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }
}
