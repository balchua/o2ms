use oauth2_test_server::{
    crypto::generate_token_string,
    models::Client,
    AppState,
};

use crate::config::model::{AppConfig, ClientConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRegistry;

impl ClientRegistry {
    #[must_use]
    pub fn section_name() -> &'static str {
        "clients"
    }
}

#[must_use]
pub fn find_enabled_client_by_id<'a>(config: &'a AppConfig, client_id: &str) -> Option<&'a ClientConfig> {
    config
        .clients
        .iter()
        .find(|client| client.enabled && client.client_id == client_id)
}

/// Seed configured clients into the upstream in-memory OAuth store.
///
/// # Errors
///
/// Returns an error when client config cannot be translated into a seeded
/// upstream client.
pub async fn seed_clients(config: &AppConfig, state: &AppState) -> Result<usize, crate::AppError> {
    let mut seeded = 0_usize;

    for client in &config.clients {
        if !client.enabled {
            tracing::info!(client_id = %client.client_id, "skipping disabled preloaded client");
            continue;
        }

        let seeded_client = to_upstream_client(config, client);
        state.store.insert_client(seeded_client).await;
        seeded += 1;
        tracing::info!(client_id = %client.client_id, "seeded preloaded client");
    }

    Ok(seeded)
}

fn to_upstream_client(config: &AppConfig, client: &ClientConfig) -> Client {
    let scope_values = if client.allowed_scopes.is_empty() {
        &client.default_scopes
    } else {
        &client.allowed_scopes
    };
    let scope = scope_values.join(" ");

    let client_secret = match (
        client.client_secret.clone(),
        client.token_endpoint_auth_method.as_str(),
    ) {
        (some_secret @ Some(_), _) => some_secret,
        (None, "none") => None,
        (None, _) => Some(generate_token_string()),
    };

    let registration_client_uri = if config.server.runtime_client_registration_enabled {
        Some(format!(
            "{}/register/{}",
            config.issuer.base_url.trim_end_matches('/'),
            client.client_id
        ))
    } else {
        None
    };

    Client {
        client_id: client.client_id.clone(),
        client_secret,
        redirect_uris: client.redirect_uris.clone(),
        grant_types: client.grant_types.clone(),
        response_types: client.response_types.clone(),
        scope,
        token_endpoint_auth_method: client.token_endpoint_auth_method.clone(),
        client_name: Some(client.client_name.clone()),
        client_uri: None,
        logo_uri: None,
        contacts: Vec::new(),
        policy_uri: None,
        tos_uri: None,
        jwks: None,
        jwks_uri: None,
        software_id: None,
        software_version: None,
        registration_access_token: None,
        registration_client_uri,
    }
}

#[cfg(test)]
mod tests {
    use super::{seed_clients, to_upstream_client, ClientRegistry};
    use crate::config::model::{AppConfig, ClientConfig};
    use oauth2_test_server::AppState;

    #[test]
    fn exposes_expected_section_name() {
        assert_eq!(ClientRegistry::section_name(), "clients");
    }

    #[test]
    fn maps_client_defaults_into_upstream_client() {
        let config = AppConfig::default();
        let client = ClientConfig {
            client_id: "api".to_string(),
            client_name: "API".to_string(),
            ..ClientConfig::default()
        };

        let upstream = to_upstream_client(&config, &client);

        assert_eq!(upstream.client_id, "api");
        assert_eq!(upstream.client_name.as_deref(), Some("API"));
        assert_eq!(upstream.scope, "openid profile email");
    }

    #[tokio::test]
    async fn seeds_enabled_clients_only() -> Result<(), Box<dyn std::error::Error>> {
        let config = AppConfig {
            clients: vec![
                ClientConfig {
                    client_id: "enabled-client".to_string(),
                    client_name: "Enabled".to_string(),
                    ..ClientConfig::default()
                },
                ClientConfig {
                    client_id: "disabled-client".to_string(),
                    client_name: "Disabled".to_string(),
                    enabled: false,
                    ..ClientConfig::default()
                },
            ],
            ..AppConfig::default()
        };
        let state = AppState::new(oauth2_test_server::IssuerConfig::default());

        let seeded_count = seed_clients(&config, &state).await?;

        assert_eq!(seeded_count, 1);
        assert!(state.store.get_client("enabled-client").await.is_some());
        assert!(state.store.get_client("disabled-client").await.is_none());
        Ok(())
    }
}
