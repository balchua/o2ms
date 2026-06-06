use oauth2_test_server::AppState;

use crate::{
    claims::merge::ClaimMergePolicy, config::model::AppConfig, error::AppError,
    gateway::GatewayRuntime, http::token_response::TokenResponsePolicy,
};

#[derive(Clone)]
pub struct WrapperState {
    pub config: AppConfig,
    pub upstream: AppState,
    pub gateway: Option<GatewayRuntime>,
    pub claim_merge_policy: ClaimMergePolicy,
    pub token_response_policy: TokenResponsePolicy,
}

impl WrapperState {
    /// Build wrapper state from validated config and upstream state.
    ///
    /// # Errors
    ///
    /// Returns an error when gateway runtime initialization fails.
    pub fn new(config: AppConfig, upstream: AppState) -> Result<Self, AppError> {
        let token_response_policy = TokenResponsePolicy::from_config(&config);
        let gateway =
            GatewayRuntime::from_config(&config.gateway).map_err(AppError::InvalidConfig)?;

        Ok(Self {
            config,
            upstream,
            gateway,
            claim_merge_policy: ClaimMergePolicy::default(),
            token_response_policy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WrapperState;
    use crate::config::model::AppConfig;
    use oauth2_test_server::AppState;

    #[test]
    fn creates_default_wrapper_state() -> Result<(), Box<dyn std::error::Error>> {
        let state = WrapperState::new(
            AppConfig::default(),
            AppState::new(oauth2_test_server::IssuerConfig::default()),
        )?;

        assert!(state.token_response_policy.default_config.emit_json_body);
        assert_eq!(state.config.server.bind_port, 8090);
        assert_eq!(
            state.claim_merge_policy.precedence(),
            ["server", "client", "user"]
        );
        assert!(state.gateway.is_none());
        Ok(())
    }
}
