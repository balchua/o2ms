use axum::{
    Json,
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::Value;

use crate::config::model::{
    AppConfig, ClientConfig, HeaderValueFormat, TokenField, TokenResponseConfig,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenResponsePolicy {
    pub default_config: TokenResponseConfig,
}

impl TokenResponsePolicy {
    #[must_use]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            default_config: config.token_response.clone(),
        }
    }

    #[must_use]
    pub fn resolved_config<'a>(
        &'a self,
        client: Option<&'a ClientConfig>,
    ) -> &'a TokenResponseConfig {
        client
            .and_then(|configured_client| configured_client.token_response_override.as_ref())
            .unwrap_or(&self.default_config)
    }

    /// Shape a token endpoint response using the resolved body/header policy.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured header name or value cannot be rendered.
    pub fn shape_response(
        &self,
        client: Option<&ClientConfig>,
        payload: &Value,
    ) -> Result<Response, crate::error::AppError> {
        let resolved = self.resolved_config(client);
        let mut response = if resolved.emit_json_body {
            Json(payload.clone()).into_response()
        } else {
            StatusCode::OK.into_response()
        };

        for header in &resolved.emit_headers {
            let Some(token_value) = extract_token_value(payload, &header.token_field) else {
                continue;
            };

            let header_name =
                HeaderName::try_from(header.header_name.as_str()).map_err(|error| {
                    crate::error::AppError::InvalidConfig(format!(
                        "token response header '{}' is invalid: {error}",
                        header.header_name
                    ))
                })?;
            let header_value =
                HeaderValue::from_str(&render_header_value(token_value, &header.value_format))
                    .map_err(|error| {
                        crate::error::AppError::InvalidConfig(format!(
                            "token response header '{}' has an invalid value: {error}",
                            header.header_name
                        ))
                    })?;

            response.headers_mut().append(header_name, header_value);
        }

        Ok(response)
    }
}

fn extract_token_value<'a>(payload: &'a Value, token_field: &TokenField) -> Option<&'a str> {
    let field_name = match token_field {
        TokenField::AccessToken => "access_token",
        TokenField::RefreshToken => "refresh_token",
        TokenField::IdToken => "id_token",
    };

    payload.get(field_name).and_then(Value::as_str)
}

fn render_header_value(token_value: &str, value_format: &HeaderValueFormat) -> String {
    match value_format {
        HeaderValueFormat::Bearer => format!("Bearer {token_value}"),
        HeaderValueFormat::Raw => token_value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use serde_json::json;

    use super::TokenResponsePolicy;
    use crate::config::model::{
        AppConfig, ClientConfig, HeaderValueFormat, TokenField, TokenHeaderConfig,
        TokenResponseConfig,
    };

    #[test]
    fn json_body_is_enabled_by_default() {
        assert!(TokenResponsePolicy::default().default_config.emit_json_body);
    }

    #[test]
    fn client_override_replaces_global_policy() {
        let policy = TokenResponsePolicy::from_config(&AppConfig {
            token_response: TokenResponseConfig {
                emit_json_body: true,
                emit_headers: vec![TokenHeaderConfig::default()],
            },
            ..AppConfig::default()
        });
        let client = ClientConfig {
            client_id: "client-a".to_string(),
            client_name: "Client A".to_string(),
            token_response_override: Some(TokenResponseConfig {
                emit_json_body: false,
                emit_headers: vec![TokenHeaderConfig {
                    header_name: "X-Auth-Token".to_string(),
                    token_field: TokenField::AccessToken,
                    value_format: HeaderValueFormat::Raw,
                }],
            }),
            ..ClientConfig::default()
        };

        let resolved = policy.resolved_config(Some(&client));

        assert!(!resolved.emit_json_body);
        assert_eq!(resolved.emit_headers[0].header_name, "X-Auth-Token");
    }

    #[tokio::test]
    async fn shaping_can_emit_header_only_response() -> Result<(), Box<dyn std::error::Error>> {
        let policy = TokenResponsePolicy::from_config(&AppConfig {
            token_response: TokenResponseConfig {
                emit_json_body: false,
                emit_headers: vec![TokenHeaderConfig::default()],
            },
            ..AppConfig::default()
        });
        let response = policy.shape_response(
            None,
            &json!({
                "access_token": "token-123",
                "token_type": "Bearer",
            }),
        )?;

        assert_eq!(
            response.headers().get("Authorization").and_then(|value| value.to_str().ok()),
            Some("Bearer token-123")
        );

        let body = to_bytes(response.into_body(), usize::MAX).await?;
        assert!(body.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn shaping_can_emit_body_and_custom_header() -> Result<(), Box<dyn std::error::Error>> {
        let policy = TokenResponsePolicy::from_config(&AppConfig {
            token_response: TokenResponseConfig {
                emit_json_body: true,
                emit_headers: vec![TokenHeaderConfig {
                    header_name: "X-Refresh-Token".to_string(),
                    token_field: TokenField::RefreshToken,
                    value_format: HeaderValueFormat::Raw,
                }],
            },
            ..AppConfig::default()
        });
        let response = policy.shape_response(
            None,
            &json!({
                "access_token": "access-123",
                "refresh_token": "refresh-123",
                "token_type": "Bearer",
            }),
        )?;

        assert_eq!(
            response
                .headers()
                .get("X-Refresh-Token")
                .and_then(|value| value.to_str().ok()),
            Some("refresh-123")
        );

        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(payload["access_token"], json!("access-123"));
        Ok(())
    }
}
