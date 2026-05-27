use axum::{
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use std::collections::{BTreeMap, HashMap};
use oauth2_test_server::{
    error::OauthError,
    handlers::{
        authorize::{authorize, AuthorizeQuery},
        device::{device_code, device_token, DeviceCodeRequest},
        discovery::{jwks, well_known_openid_configuration},
        error::error_page,
        introspect::introspect,
        register::{get_client, register_client},
        revoke::revoke,
        token::TokenRequest,
        userinfo::userinfo,
    },
    models::Token,
};
use chrono::{Duration, Utc};

use crate::{
    app::state::WrapperState,
    claims::issue::{issue_access_token, issue_optional_id_token},
    registry::clients::find_enabled_client_by_id,
};

pub async fn discovery(State(state): State<WrapperState>) -> Response {
    well_known_openid_configuration(State(state.upstream.clone()))
        .await
        .into_response()
}

pub async fn jwks_doc(State(state): State<WrapperState>) -> Response {
    jwks(State(state.upstream.clone())).await.into_response()
}

pub async fn authorize_flow(
    State(state): State<WrapperState>,
    Query(params): Query<AuthorizeQuery>,
) -> Response {
    authorize(State(state.upstream.clone()), Query(params))
        .await
        .into_response()
}

/// Proxy the dynamic client registration endpoint to the embedded OAuth server.
///
/// # Errors
///
/// Returns any registration error from the embedded OAuth server.
pub async fn register_client_proxy(
    State(state): State<WrapperState>,
    Json(metadata): Json<serde_json::Value>,
) -> Result<Response, OauthError> {
    register_client(State(state.upstream.clone()), Json(metadata))
        .await
        .map(IntoResponse::into_response)
}

pub async fn get_client_proxy(
    State(state): State<WrapperState>,
    Path(client_id): Path<String>,
) -> Response {
    get_client(State(state.upstream.clone()), Path(client_id))
        .await
        .into_response()
}

pub async fn introspect_proxy(
    State(state): State<WrapperState>,
    Form(form): Form<BTreeMap<String, String>>,
) -> Response {
    introspect(State(state.upstream.clone()), Form(HashMap::from_iter(form)))
        .await
        .into_response()
}

pub async fn revoke_proxy(
    State(state): State<WrapperState>,
    Form(form): Form<BTreeMap<String, String>>,
) -> Response {
    revoke(State(state.upstream.clone()), Form(HashMap::from_iter(form)))
        .await
        .into_response()
}

/// Proxy the userinfo endpoint to the embedded OAuth server.
///
/// # Errors
///
/// Returns any token validation error from the embedded OAuth server.
pub async fn userinfo_proxy(
    State(state): State<WrapperState>,
    headers: HeaderMap,
) -> Result<Response, OauthError> {
    userinfo(headers, State(state.upstream.clone()))
        .await
        .map(IntoResponse::into_response)
}

/// Proxy the device authorization endpoint to the embedded OAuth server.
///
/// # Errors
///
/// Returns any device authorization error from the embedded OAuth server.
pub async fn device_code_proxy(
    State(state): State<WrapperState>,
    Form(form): Form<DeviceCodeRequest>,
) -> Result<Response, OauthError> {
    device_code(State(state.upstream.clone()), Form(form))
        .await
        .map(IntoResponse::into_response)
}

/// Proxy the device token endpoint to the embedded OAuth server.
///
/// # Errors
///
/// Returns any device token polling error from the embedded OAuth server.
pub async fn device_token_proxy(
    State(state): State<WrapperState>,
    Form(form): Form<oauth2_test_server::models::DeviceTokenRequest>,
) -> Result<Response, OauthError> {
    device_token(State(state.upstream.clone()), Form(form))
        .await
        .map(IntoResponse::into_response)
}

pub async fn error_page_proxy(
    Query(params): Query<BTreeMap<String, String>>,
) -> Response {
    error_page(Query(HashMap::from_iter(params))).await.into_response()
}

/// Wrapper-owned token endpoint that injects custom claims into issued JWTs.
///
/// # Errors
///
/// Returns token issuance, grant validation, or claim-encoding errors.
pub async fn token_endpoint(
    State(state): State<WrapperState>,
    headers: HeaderMap,
    Form(form): Form<TokenRequest>,
) -> Result<Response, OauthError> {
    let _ = headers;
    match form.grant_type.as_str() {
        "authorization_code" => handle_authorization_code(state, form).await,
        "refresh_token" => handle_refresh_token(state, form).await,
        "client_credentials" => handle_client_credentials(state, form).await,
        _ => Err(OauthError::UnsupportedGrantType),
    }
}

async fn handle_authorization_code(
    state: WrapperState,
    form: TokenRequest,
) -> Result<Response, OauthError> {
    use base64::{engine::general_purpose, Engine};
    use sha2::Digest;

    let code = form.code.as_deref().unwrap_or("");
    let code_obj = state
        .upstream
        .store
        .remove_code(code)
        .await
        .ok_or(OauthError::InvalidGrant)?;

    if code_obj.expires_at < Utc::now() {
        return Err(OauthError::InvalidGrant);
    }

    if let (Some(challenge), Some(verifier)) = (&code_obj.code_challenge, &form.code_verifier) {
        let method = code_obj.code_challenge_method.as_deref().unwrap_or("plain");
        let computed = if method == "S256" {
            general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
        } else {
            verifier.clone()
        };
        if computed != *challenge {
            return Err(OauthError::InvalidGrant);
        }
    }

    let refresh_token = oauth2_test_server::crypto::generate_token_string();
    let jwt = issue_access_token(
        &state,
        &code_obj.client_id,
        &code_obj.user_id,
        &code_obj.scope,
    )?;
    let id_token = issue_optional_id_token(
        &state,
        &code_obj.client_id,
        &code_obj.user_id,
        &jwt,
        Some(code),
        code_obj.nonce.as_deref(),
        &code_obj.scope,
    )?;

    let token = Token {
        access_token: jwt.clone(),
        refresh_token: Some(refresh_token.clone()),
        client_id: code_obj.client_id.clone(),
        scope: code_obj.scope.clone(),
        expires_at: Utc::now()
            + Duration::seconds(
                i64::try_from(state.upstream.config.access_token_expires_in)
                    .map_err(|_| OauthError::ServerError)?,
            ),
        user_id: code_obj.user_id.clone(),
        revoked: false,
    };

    state.upstream.store.insert_token(jwt.clone(), token.clone()).await;
    state
        .upstream
        .store
        .insert_refresh_token(refresh_token.clone(), token)
        .await;

    let mut response = serde_json::json!({
        "access_token": jwt,
        "token_type": "Bearer",
        "expires_in": state.upstream.config.access_token_expires_in,
        "refresh_token": refresh_token,
        "scope": code_obj.scope
    });

    if let Some(id_token) = id_token {
        response["id_token"] = serde_json::Value::String(id_token);
    }
    if let Some(state_value) = &code_obj.state {
        response["state"] = serde_json::Value::String(state_value.clone());
    }

    let client = find_enabled_client_by_id(&state.config, &code_obj.client_id);
    state
        .token_response_policy
        .shape_response(client, &response)
        .map_err(|_| OauthError::ServerError)
}

async fn handle_refresh_token(
    state: WrapperState,
    form: TokenRequest,
) -> Result<Response, OauthError> {
    let rt = form.refresh_token.as_deref().unwrap_or("");
    let mut token = state
        .upstream
        .store
        .get_refresh_token(rt)
        .await
        .ok_or(OauthError::InvalidGrant)?;

    if token.revoked {
        return Err(OauthError::InvalidGrant);
    }

    let new_access_token = issue_access_token(
        &state,
        &token.client_id,
        &token.user_id,
        &token.scope,
    )?;
    let new_refresh_token = oauth2_test_server::crypto::generate_token_string();

    let new_token = Token {
        access_token: new_access_token.clone(),
        refresh_token: Some(new_refresh_token.clone()),
        client_id: token.client_id.clone(),
        scope: token.scope.clone(),
        expires_at: Utc::now()
            + Duration::seconds(
                i64::try_from(state.upstream.config.access_token_expires_in)
                    .map_err(|_| OauthError::ServerError)?,
            ),
        user_id: token.user_id.clone(),
        revoked: false,
    };

    state
        .upstream
        .store
        .insert_token(new_access_token.clone(), new_token.clone())
        .await;
    state
        .upstream
        .store
        .insert_refresh_token(new_refresh_token.clone(), new_token)
        .await;

    token.revoked = true;
    state.upstream.store.update_refresh_token(rt, token.clone()).await;

    let response = serde_json::json!({
        "access_token": new_access_token,
        "token_type": "Bearer",
        "expires_in": state.upstream.config.access_token_expires_in,
        "refresh_token": new_refresh_token,
        "scope": token.scope
    });
    let client = find_enabled_client_by_id(&state.config, &token.client_id);
    state
        .token_response_policy
        .shape_response(client, &response)
        .map_err(|_| OauthError::ServerError)
}

async fn handle_client_credentials(
    state: WrapperState,
    form: TokenRequest,
) -> Result<Response, OauthError> {
    use std::collections::HashSet;

    let client_id = form.client_id.as_deref().unwrap_or("");
    let client = state
        .upstream
        .store
        .get_client(client_id)
        .await
        .ok_or(OauthError::InvalidClient)?;

    let requested_scopes: HashSet<String> = form
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(ToString::to_string)
        .collect();

    if let Some(requested_scope) = form.scope.as_deref() {
        if let Err(error) = state.upstream.config.validate_scope(requested_scope) {
            return Err(OauthError::InvalidScope(error));
        }

        let client_scopes: HashSet<_> = client.scope.split_whitespace().collect();
        let requested_scopes_set: HashSet<_> = requested_scope.split_whitespace().collect();
        let not_permitted: Vec<_> = requested_scopes_set
            .difference(&client_scopes)
            .copied()
            .collect();

        if !not_permitted.is_empty() {
            return Err(OauthError::InvalidScope(format!(
                "Client not authorized for scopes: {}",
                not_permitted.join(" ")
            )));
        }
    }

    let registered_scopes: HashSet<String> =
        client.scope.split_whitespace().map(ToString::to_string).collect();
    let granted_scopes: Vec<String> = requested_scopes
        .intersection(&registered_scopes)
        .cloned()
        .collect();

    if granted_scopes.is_empty() && !requested_scopes.is_empty() {
        return Err(OauthError::InvalidScope(
            "Requested scopes not allowed for this client".to_string(),
        ));
    }

    let final_scope = if requested_scopes.is_empty() {
        client.scope.clone()
    } else {
        granted_scopes.join(" ")
    };

    let access_token = issue_access_token(&state, client_id, "client", &final_scope)?;

    let response = serde_json::json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": state.upstream.config.access_token_expires_in,
        "scope": final_scope
    });
    let client = find_enabled_client_by_id(&state.config, client_id);
    state
        .token_response_policy
        .shape_response(client, &response)
        .map_err(|_| OauthError::ServerError)
}
