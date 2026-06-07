use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose, Engine as _};
use oauth2_test_server::handlers::token::TokenRequest;

use crate::app::state::WrapperState;

pub async fn validate_client_auth(
    state: &WrapperState,
    headers: &HeaderMap,
    form: &TokenRequest,
) -> Result<(), Response> {
    let client_id = resolve_client_id(headers, form);
    let Some(client_id) = client_id else {
        return Err(client_auth_error());
    };

    let Some(client) = state.upstream.store.get_client(&client_id).await else {
        return Err(client_auth_error());
    };

    match client.token_endpoint_auth_method.as_str() {
        "none" => {}
        "client_secret_basic" => {
            let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) else {
                return Err(client_auth_error());
            };
            let Ok(auth_value) = auth_header.to_str() else {
                return Err(client_auth_error());
            };
            let Some(encoded) = auth_value.strip_prefix("Basic ") else {
                return Err(client_auth_error());
            };
            let Ok(decoded) = general_purpose::STANDARD.decode(encoded) else {
                return Err(client_auth_error());
            };
            let Ok(credentials) = String::from_utf8(decoded) else {
                return Err(client_auth_error());
            };
            let Some((basic_client_id, basic_secret)) = credentials.split_once(':') else {
                return Err(client_auth_error());
            };
            if basic_client_id != client_id {
                return Err(client_auth_error());
            }
            let Some(expected_secret) = &client.client_secret else {
                return Err(client_auth_error());
            };
            if basic_secret != expected_secret.as_str() {
                return Err(client_auth_error());
            }
        }
        "client_secret_post" => {
            let Some(secret) = &form._client_secret else {
                return Err(client_auth_error());
            };
            let Some(expected_secret) = &client.client_secret else {
                return Err(client_auth_error());
            };
            if secret != expected_secret.as_str() {
                return Err(client_auth_error());
            }
        }
        _ => {
            return Err(client_auth_error());
        }
    }

    Ok(())
}

fn resolve_client_id(headers: &HeaderMap, form: &TokenRequest) -> Option<String> {
    if let Some(client_id) = form.client_id.as_deref()
        && !client_id.is_empty()
    {
        return Some(client_id.to_string());
    }
    let auth_header = headers.get(axum::http::header::AUTHORIZATION)?;
    let auth_value = auth_header.to_str().ok()?;
    let encoded = auth_value.strip_prefix("Basic ")?;
    let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let (client_id, _) = credentials.split_once(':')?;
    if client_id.is_empty() { None } else { Some(client_id.to_string()) }
}

fn client_auth_error() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "invalid_client"})),
    )
        .into_response()
}