use oauth2_test_server::AppState;

use crate::{
    claims::merge::ClaimMergePolicy, config::model::AppConfig,
    http::token_response::TokenResponsePolicy,
};

#[derive(Clone)]
pub struct WrapperState {
    pub config: AppConfig,
    pub upstream: AppState,
    pub claim_merge_policy: ClaimMergePolicy,
    pub token_response_policy: TokenResponsePolicy,
}

impl WrapperState {
    #[must_use]
    pub fn new(config: AppConfig, upstream: AppState) -> Self {
        let token_response_policy = TokenResponsePolicy::from_config(&config);

        Self {
            config,
            upstream,
            claim_merge_policy: ClaimMergePolicy::default(),
            token_response_policy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WrapperState;
    use crate::config::model::AppConfig;
    use oauth2_test_server::AppState;

    #[test]
    fn creates_default_wrapper_state() {
        let state = WrapperState::new(
            AppConfig::default(),
            AppState::new(oauth2_test_server::IssuerConfig::default()),
        );

        assert!(state.token_response_policy.default_config.emit_json_body);
        assert_eq!(state.config.server.bind_port, 8090);
        assert_eq!(state.claim_merge_policy.precedence(), ["server", "client", "user"]);
    }
}
