use std::collections::HashSet;

use http::header::HeaderName;
use url::Url;

use crate::{
    claims::protect::PROTECTED_CLAIMS,
    config::model::{AppConfig, ClientConfig, UserConfig},
    error::AppError,
};

/// Validate the application config.
///
/// # Errors
///
/// Returns an error when a required field is missing or empty.
pub fn validate(config: &AppConfig) -> Result<(), AppError> {
    if config.server.bind_host.trim().is_empty() {
        return Err(AppError::InvalidConfig(
            "server.bind_host must not be empty".to_string(),
        ));
    }

    if config.server.log_level.trim().is_empty() {
        return Err(AppError::InvalidConfig(
            "server.log_level must not be empty".to_string(),
        ));
    }

    let issuer_url = Url::parse(&config.issuer.base_url).map_err(|error| {
        AppError::InvalidConfig(format!("issuer.base_url must be a valid URL: {error}"))
    })?;
    if issuer_url.host_str().is_none() {
        return Err(AppError::InvalidConfig(
            "issuer.base_url must include a host".to_string(),
        ));
    }
    if !(issuer_url.path().is_empty() || issuer_url.path() == "/") {
        return Err(AppError::InvalidConfig(
            "issuer.base_url paths are not supported yet; use a root base URL".to_string(),
        ));
    }

    validate_ttls(config)?;
    validate_oauth_capabilities(config)?;
    validate_token_headers(config)?;
    validate_clients(config)?;
    validate_users(config)?;
    validate_claim_templates(config)?;

    Ok(())
}

fn validate_oauth_capabilities(config: &AppConfig) -> Result<(), AppError> {
    let supported_token_auth_methods = [
        "client_secret_basic",
        "client_secret_post",
        "none",
        "private_key_jwt",
    ];
    for method in &config.oauth.token_endpoint_auth_methods {
        if !supported_token_auth_methods.contains(&method.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "oauth.token_endpoint_auth_methods contains unsupported method '{method}'"
            )));
        }
    }

    let supported_challenge_methods = ["plain", "S256"];
    for method in &config.oauth.code_challenge_methods {
        if !supported_challenge_methods.contains(&method.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "oauth.code_challenge_methods contains unsupported method '{method}'"
            )));
        }
    }

    if config.oauth.signing_algorithm != "RS256" {
        return Err(AppError::InvalidConfig(
            "oauth.signing_algorithm currently only supports RS256".to_string(),
        ));
    }

    if config.oauth.signing_key_strategy != "ephemeral_rsa" {
        return Err(AppError::InvalidConfig(
            "oauth.signing_key_strategy currently only supports ephemeral_rsa".to_string(),
        ));
    }

    Ok(())
}

fn validate_ttls(config: &AppConfig) -> Result<(), AppError> {
    if config.oauth.access_token_ttl_seconds == 0 {
        return Err(AppError::InvalidConfig(
            "oauth.access_token_ttl_seconds must be greater than zero".to_string(),
        ));
    }

    if config.oauth.refresh_token_ttl_seconds == 0 {
        return Err(AppError::InvalidConfig(
            "oauth.refresh_token_ttl_seconds must be greater than zero".to_string(),
        ));
    }

    if config.oauth.authorization_code_ttl_seconds == 0 {
        return Err(AppError::InvalidConfig(
            "oauth.authorization_code_ttl_seconds must be greater than zero".to_string(),
        ));
    }

    Ok(())
}

fn validate_token_headers(config: &AppConfig) -> Result<(), AppError> {
    validate_token_response_config(&config.token_response, "token_response")
}

fn validate_token_response_config(
    token_response: &crate::config::model::TokenResponseConfig,
    path: &str,
) -> Result<(), AppError> {
    if !token_response.emit_json_body && token_response.emit_headers.is_empty() {
        return Err(AppError::InvalidConfig(format!(
            "{path} must emit either the JSON body or at least one header"
        )));
    }

    for header in &token_response.emit_headers {
        HeaderName::from_bytes(header.header_name.as_bytes()).map_err(|error| {
            AppError::InvalidConfig(format!(
                "{path}.emit_headers header_name '{}' is invalid: {error}",
                header.header_name
            ))
        })?;
    }

    Ok(())
}

fn validate_clients(config: &AppConfig) -> Result<(), AppError> {
    let mut client_ids = HashSet::new();
    let declared_users: HashSet<&str> = config.users.iter().map(|user| user.user_id.as_str()).collect();

    for client in &config.clients {
        validate_client_fields(client, config)?;
        if !client_ids.insert(client.client_id.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "duplicate client_id '{}'",
                client.client_id
            )));
        }

        for linked_user in &client.linked_users {
            if !declared_users.contains(linked_user.as_str()) {
                return Err(AppError::InvalidConfig(format!(
                    "client '{}' references unknown linked user '{}'",
                    client.client_id, linked_user
                )));
            }
        }

        validate_claim_keys(
            &client.custom_claims.keys().map(String::as_str).collect::<Vec<_>>(),
            "clients[].custom_claims",
        )?;
        validate_template_refs(&client.claims_template_refs, config, "client")?;
    }

    Ok(())
}

fn validate_client_fields(client: &ClientConfig, config: &AppConfig) -> Result<(), AppError> {
    if client.client_id.trim().is_empty() {
        return Err(AppError::InvalidConfig(
            "clients[].client_id must not be empty".to_string(),
        ));
    }

    if client.client_name.trim().is_empty() {
        return Err(AppError::InvalidConfig(format!(
            "client '{}' must have a client_name",
            client.client_id
        )));
    }

    validate_supported_values(
        &client.grant_types,
        &config.oauth.supported_grant_types,
        "grant_types",
        &client.client_id,
    )?;
    validate_supported_values(
        &client.response_types,
        &config.oauth.supported_response_types,
        "response_types",
        &client.client_id,
    )?;
    validate_supported_values(
        &client.allowed_scopes,
        &config.oauth.supported_scopes,
        "allowed_scopes",
        &client.client_id,
    )?;
    validate_supported_values(
        &client.default_scopes,
        &config.oauth.supported_scopes,
        "default_scopes",
        &client.client_id,
    )?;
    validate_default_scopes_subset(client)?;

    if let Some(token_response_override) = &client.token_response_override {
        validate_token_response_config(
            token_response_override,
            &format!("client '{}' token_response_override", client.client_id),
        )?;
    }

    Ok(())
}

fn validate_default_scopes_subset(client: &ClientConfig) -> Result<(), AppError> {
    let allowed_scopes: HashSet<&str> = client.allowed_scopes.iter().map(String::as_str).collect();
    if allowed_scopes.is_empty() {
        return Ok(());
    }

    for scope in &client.default_scopes {
        if !allowed_scopes.contains(scope.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "client '{}' default scope '{}' is not included in allowed_scopes",
                client.client_id, scope
            )));
        }
    }

    Ok(())
}

fn validate_users(config: &AppConfig) -> Result<(), AppError> {
    let mut user_ids = HashSet::new();

    for user in &config.users {
        validate_user_fields(user, config)?;
        if !user_ids.insert(user.user_id.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "duplicate user_id '{}'",
                user.user_id
            )));
        }

        validate_claim_keys(
            &user.custom_claims.keys().map(String::as_str).collect::<Vec<_>>(),
            "users[].custom_claims",
        )?;
        validate_template_refs(&user.claims_template_refs, config, "user")?;
    }

    Ok(())
}

fn validate_user_fields(user: &UserConfig, config: &AppConfig) -> Result<(), AppError> {
    if user.user_id.trim().is_empty() {
        return Err(AppError::InvalidConfig(
            "users[].user_id must not be empty".to_string(),
        ));
    }

    if user.sub.trim().is_empty() {
        return Err(AppError::InvalidConfig(format!(
            "user '{}' must have a sub",
            user.user_id
        )));
    }

    validate_supported_values(
        &user.default_scopes,
        &config.oauth.supported_scopes,
        "default_scopes",
        &user.user_id,
    )
}

fn validate_claim_templates(config: &AppConfig) -> Result<(), AppError> {
    for (template_name, claims) in &config.claims_templates {
        validate_claim_keys(
            &claims.keys().map(String::as_str).collect::<Vec<_>>(),
            &format!("claims_templates.{template_name}"),
        )?;
    }

    Ok(())
}

fn validate_claim_keys(keys: &[&str], field_path: &str) -> Result<(), AppError> {
    for key in keys {
        if PROTECTED_CLAIMS.contains(key) {
            return Err(AppError::InvalidConfig(format!(
                "{field_path} cannot override protected claim '{key}'"
            )));
        }
    }

    Ok(())
}

fn validate_template_refs(
    template_refs: &[String],
    config: &AppConfig,
    subject: &str,
) -> Result<(), AppError> {
    for template_ref in template_refs {
        if !config.claims_templates.contains_key(template_ref) {
            return Err(AppError::InvalidConfig(format!(
                "{subject} references unknown claims template '{template_ref}'"
            )));
        }
    }

    Ok(())
}

fn validate_supported_values(
    requested: &[String],
    supported: &[String],
    field_name: &str,
    subject_id: &str,
) -> Result<(), AppError> {
    let supported_values: HashSet<&str> = supported.iter().map(String::as_str).collect();
    for value in requested {
        if !supported_values.contains(value.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "'{value}' in {field_name} for '{subject_id}' is not declared in oauth.{field_name}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::{loader::load_from_yaml, model::AppConfig, validate::validate};

    #[test]
    fn rejects_empty_bind_host() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AppConfig::default();
        config.server.bind_host.clear();

        let result = validate(&config);
        assert!(result.is_err());
        let error_text = match result {
            Err(error) => error.to_string(),
            Ok(()) => return Err("expected bind_host validation failure".into()),
        };
        assert!(error_text.contains("server.bind_host"));
        Ok(())
    }

    #[test]
    fn rejects_invalid_issuer_url() {
        let mut config = AppConfig::default();
        config.issuer.base_url = "not a url".to_string();

        let result = validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_issuer_url_with_path() {
        let mut config = AppConfig::default();
        config.issuer.base_url = "http://127.0.0.1:8090/mock".to_string();

        let result = validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unsupported_signing_algorithm() {
        let mut config = AppConfig::default();
        config.oauth.signing_algorithm = "HS256".to_string();

        let result = validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_duplicate_client_ids() {
        let result = load_from_yaml(
            r"
issuer:
  base_url: http://127.0.0.1:8090
clients:
  - client_id: duplicate
    client_name: A
  - client_id: duplicate
    client_name: B
",
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_header_name() {
        let result = load_from_yaml(
            r#"
issuer:
  base_url: http://127.0.0.1:8090
token_response:
  emit_headers:
    - header_name: "bad header"
      token_field: access_token
      value_format: bearer
"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_empty_token_response_outputs() {
        let result = load_from_yaml(
            r"
issuer:
  base_url: http://127.0.0.1:8090
token_response:
  emit_json_body: false
  emit_headers: []
",
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_protected_claim_override() {
        let result = load_from_yaml(
            r"
issuer:
  base_url: http://127.0.0.1:8090
users:
  - user_id: demo
    sub: demo
    username: demo
    email: demo@example.com
    display_name: Demo
    custom_claims:
      iss: bad
",
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_default_scope_outside_allowed_scopes() {
        let result = load_from_yaml(
            r"
issuer:
  base_url: http://127.0.0.1:8090
clients:
  - client_id: test-client
    client_name: Test Client
    allowed_scopes:
      - profile
    default_scopes:
      - openid
",
        );

        assert!(result.is_err());
    }
}
