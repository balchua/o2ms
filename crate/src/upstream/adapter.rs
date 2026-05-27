use std::collections::HashSet;

use oauth2_test_server::{AppState, IssuerConfig};
use url::Url;

use crate::{config::model::AppConfig, registry::users::effective_default_user_id};

#[must_use]
pub fn build_upstream_state(config: &AppConfig, actual_port: u16) -> AppState {
    let parsed_base_url = Url::parse(&config.issuer.base_url).ok();
    let scheme = parsed_base_url
        .as_ref()
        .map_or("http".to_string(), |url| url.scheme().to_string());
    let host = parsed_base_url.as_ref().and_then(Url::host_str).map_or_else(
        || config.server.bind_host.clone(),
        std::string::ToString::to_string,
    );

    let issuer_config = IssuerConfig {
        scheme,
        host,
        port: actual_port,
        allowed_origins: config.server.cors_allowed_origins.clone(),
        require_state: config.oauth.require_state,
        default_user_id: effective_default_user_id(config),
        access_token_expires_in: config.oauth.access_token_ttl_seconds,
        refresh_token_expires_in: config.oauth.refresh_token_ttl_seconds,
        authorization_code_expires_in: config.oauth.authorization_code_ttl_seconds,
        cleanup_interval_secs: config.oauth.cleanup_interval_seconds,
        grant_types_supported: config.oauth.supported_grant_types.iter().cloned().collect::<HashSet<_>>(),
        response_types_supported: config.oauth.supported_response_types.iter().cloned().collect::<HashSet<_>>(),
        scopes_supported: config.oauth.supported_scopes.iter().cloned().collect::<HashSet<_>>(),
        claims_supported: config.oauth.supported_claims.clone(),
        token_endpoint_auth_methods_supported: config
            .oauth
            .token_endpoint_auth_methods
            .iter()
            .cloned()
            .collect::<HashSet<_>>(),
        code_challenge_methods_supported: config
            .oauth
            .code_challenge_methods
            .iter()
            .cloned()
            .collect::<HashSet<_>>(),
        id_token_signing_alg_values_supported: vec![config.oauth.signing_algorithm.clone()],
        ..Default::default()
    };

    AppState::new(issuer_config)
}

pub fn build_upstream_router(state: AppState) -> axum::Router {
    oauth2_test_server::router::build_router(state)
}

#[cfg(test)]
mod tests {
    use super::build_upstream_state;
    use crate::config::model::{AppConfig, UserConfig};

    #[test]
    fn maps_runtime_and_oauth_settings_to_upstream_state() {
        let mut config = AppConfig::default();
        config.server.bind_host = "0.0.0.0".to_string();
        config.server.cors_allowed_origins = vec!["http://localhost:8081".to_string()];
        config.oauth.require_state = false;
        config.oauth.access_token_ttl_seconds = 120;
        config.oauth.token_endpoint_auth_methods = vec!["none".to_string()];
        config.oauth.code_challenge_methods = vec!["S256".to_string()];

        let state = build_upstream_state(&config, 9191);

        assert_eq!(state.config.host, "127.0.0.1");
        assert_eq!(state.config.port, 9191);
        assert!(!state.config.require_state);
        assert_eq!(state.config.access_token_expires_in, 120);
        assert!(state.config.token_endpoint_auth_methods_supported.contains("none"));
        assert!(state.config.code_challenge_methods_supported.contains("S256"));
        assert_eq!(state.base_url, "http://127.0.0.1:9191");
    }

    #[test]
    fn uses_first_enabled_user_as_upstream_default_user() {
        let config = AppConfig {
            users: vec![
                UserConfig {
                    user_id: "demo-user".to_string(),
                    sub: "demo-sub".to_string(),
                    enabled: true,
                    ..UserConfig::default()
                },
            ],
            ..AppConfig::default()
        };

        let state = build_upstream_state(&config, 9191);

        assert_eq!(state.config.default_user_id, "demo-sub");
    }
}
