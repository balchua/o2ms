use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub issuer: IssuerConfig,
    pub oauth: OauthConfig,
    pub token_response: TokenResponseConfig,
    pub clients: Vec<ClientConfig>,
    pub users: Vec<UserConfig>,
    pub claims_templates: BTreeMap<String, BTreeMap<String, Value>>,
    pub admin: AdminConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub log_level: String,
    pub cors_allowed_origins: Vec<String>,
    pub startup_mode: StartupMode,
    pub health_endpoint_enabled: bool,
    pub runtime_client_registration_enabled: bool,
    pub deterministic_seed: Option<u64>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8090,
            log_level: "info".to_string(),
            cors_allowed_origins: Vec::new(),
            startup_mode: StartupMode::Foreground,
            health_endpoint_enabled: true,
            runtime_client_registration_enabled: true,
            deterministic_seed: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    #[default]
    Foreground,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct IssuerConfig {
    pub base_url: String,
}

impl Default for IssuerConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8090".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OauthConfig {
    pub require_state: bool,
    pub pkce_required: bool,
    pub access_token_ttl_seconds: u64,
    pub refresh_token_ttl_seconds: u64,
    pub authorization_code_ttl_seconds: u64,
    pub cleanup_interval_seconds: u64,
    pub supported_grant_types: Vec<String>,
    pub supported_response_types: Vec<String>,
    pub supported_scopes: Vec<String>,
    pub supported_claims: Vec<String>,
    pub token_endpoint_auth_methods: Vec<String>,
    pub code_challenge_methods: Vec<String>,
    pub signing_algorithm: String,
    pub signing_key_strategy: String,
}

impl Default for OauthConfig {
    fn default() -> Self {
        Self {
            require_state: true,
            pkce_required: false,
            access_token_ttl_seconds: 3600,
            refresh_token_ttl_seconds: 2_592_000,
            authorization_code_ttl_seconds: 600,
            cleanup_interval_seconds: 300,
            supported_grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
                "client_credentials".to_string(),
            ],
            supported_response_types: vec!["code".to_string()],
            supported_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
                "offline_access".to_string(),
            ],
            supported_claims: vec![
                "sub".to_string(),
                "name".to_string(),
                "email".to_string(),
            ],
            token_endpoint_auth_methods: vec![
                "client_secret_basic".to_string(),
                "client_secret_post".to_string(),
                "none".to_string(),
                "private_key_jwt".to_string(),
            ],
            code_challenge_methods: vec!["plain".to_string(), "S256".to_string()],
            signing_algorithm: "RS256".to_string(),
            signing_key_strategy: "ephemeral_rsa".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TokenResponseConfig {
    pub emit_json_body: bool,
    pub emit_headers: Vec<TokenHeaderConfig>,
}

impl Default for TokenResponseConfig {
    fn default() -> Self {
        Self {
            emit_json_body: true,
            emit_headers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TokenHeaderConfig {
    pub header_name: String,
    pub token_field: TokenField,
    pub value_format: HeaderValueFormat,
}

impl Default for TokenHeaderConfig {
    fn default() -> Self {
        Self {
            header_name: "Authorization".to_string(),
            token_field: TokenField::AccessToken,
            value_format: HeaderValueFormat::Bearer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenField {
    AccessToken,
    RefreshToken,
    IdToken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeaderValueFormat {
    Bearer,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_name: String,
    pub enabled: bool,
    pub client_secret: Option<String>,
    pub token_endpoint_auth_method: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub allowed_scopes: Vec<String>,
    pub default_scopes: Vec<String>,
    pub linked_users: Vec<String>,
    pub token_response_override: Option<TokenResponseConfig>,
    pub claims_template_refs: Vec<String>,
    pub custom_claims: BTreeMap<String, Value>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_name: String::new(),
            enabled: true,
            client_secret: None,
            token_endpoint_auth_method: "none".to_string(),
            redirect_uris: Vec::new(),
            grant_types: vec!["client_credentials".to_string()],
            response_types: vec!["code".to_string()],
            allowed_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            default_scopes: vec!["openid".to_string()],
            linked_users: Vec::new(),
            token_response_override: None,
            claims_template_refs: Vec::new(),
            custom_claims: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct UserConfig {
    pub user_id: String,
    pub sub: String,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub enabled: bool,
    pub default_scopes: Vec<String>,
    pub roles: Vec<String>,
    pub groups: Vec<String>,
    pub claims_template_refs: Vec<String>,
    pub custom_claims: BTreeMap<String, Value>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            sub: String::new(),
            username: String::new(),
            email: String::new(),
            display_name: String::new(),
            enabled: true,
            default_scopes: vec!["openid".to_string(), "profile".to_string()],
            roles: Vec::new(),
            groups: Vec::new(),
            claims_template_refs: Vec::new(),
            custom_claims: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AdminConfig {
    pub reset_endpoint_enabled: bool,
    pub list_clients_endpoint_enabled: bool,
    pub config_endpoint_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, HeaderValueFormat, TokenField};

    #[test]
    fn default_config_uses_localhost_defaults() {
        let config = AppConfig::default();

        assert_eq!(config.server.bind_host, "127.0.0.1");
        assert_eq!(config.server.bind_port, 8090);
        assert_eq!(config.server.startup_mode, super::StartupMode::Foreground);
        assert_eq!(config.issuer.base_url, "http://127.0.0.1:8090");
        assert!(config.token_response.emit_json_body);
        assert_eq!(config.oauth.signing_algorithm, "RS256");
    }

    #[test]
    fn yaml_parsing_applies_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config: AppConfig = serde_yaml::from_str(
            r"
server:
  bind_port: 9191
clients:
  - client_id: test-client
    client_name: Test Client
users:
  - user_id: demo
    sub: demo
    username: demo
    email: demo@example.com
    display_name: Demo
",
        )?;

        assert_eq!(config.server.bind_host, "127.0.0.1");
        assert_eq!(config.server.bind_port, 9191);
        assert_eq!(config.clients[0].token_endpoint_auth_method, "none");
        assert_eq!(config.token_response.emit_headers.len(), 0);
        Ok(())
    }

    #[test]
    fn token_header_defaults_are_stable() {
        let config = AppConfig::default();

        let header = super::TokenHeaderConfig::default();
        assert!(config.token_response.emit_json_body);
        assert_eq!(header.header_name, "Authorization");
        assert_eq!(header.token_field, TokenField::AccessToken);
        assert_eq!(header.value_format, HeaderValueFormat::Bearer);
    }
}
