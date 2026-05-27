use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, Header};
use oauth2_test_server::{
    crypto::{calculate_at_hash, calculate_c_hash, issue_id_token},
    error::OauthError,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    app::state::WrapperState,
    claims::merge::claims_for_subject,
};

/// Issue an access token with merged custom claims.
///
/// # Errors
///
/// Returns an error when claim timestamps cannot be represented safely or the
/// JWT cannot be encoded.
pub fn issue_access_token(
    state: &WrapperState,
    client_id: &str,
    subject: &str,
    scope: &str,
) -> Result<String, OauthError> {
    let issued_at = current_timestamp_usize()?;
    let expires_at = expiry_timestamp_usize(state.upstream.config.access_token_expires_in)?;

    let mut claims = claims_for_subject(&state.config, client_id, subject);
    claims.insert("iss".to_string(), Value::String(state.upstream.base_url.clone()));
    claims.insert("sub".to_string(), Value::String(subject.to_string()));
    claims.insert("aud".to_string(), Value::String(client_id.to_string()));
    claims.insert("exp".to_string(), Value::Number(expires_at.into()));
    claims.insert("iat".to_string(), Value::Number(issued_at.into()));
    claims.insert("scope".to_string(), Value::String(scope.to_string()));
    claims.insert("typ".to_string(), Value::String("Bearer".to_string()));
    claims.insert("jti".to_string(), Value::String(Uuid::new_v4().to_string()));
    claims.insert("azp".to_string(), Value::String(client_id.to_string()));
    claims.insert(
        "sid".to_string(),
        Value::String(format!("sid-{}", Uuid::new_v4())),
    );
    claims.insert("auth_time".to_string(), Value::Number(issued_at.into()));

    let mut header = Header::new(Algorithm::RS256);
    header.typ = Some("JWT".to_string());
    header.kid = Some(state.upstream.keys.kid.clone());

    encode(&header, &claims, &state.upstream.keys.encoding).map_err(|_| OauthError::ServerError)
}

/// Issue an ID token when the requested scope includes `openid`.
///
/// # Errors
///
/// Returns an error when the token lifetime cannot be represented safely or the
/// ID token cannot be encoded.
pub fn issue_optional_id_token(
    state: &WrapperState,
    client_id: &str,
    subject: &str,
    access_token: &str,
    authorization_code: Option<&str>,
    nonce: Option<&str>,
    scope: &str,
) -> Result<Option<String>, OauthError> {
    if !scope.split_whitespace().any(|value| value == "openid") {
        return Ok(None);
    }

    let mut user_claims = claims_for_subject(&state.config, client_id, subject);
    if user_claims.is_empty() {
        user_claims = Map::new();
    }

    let at_hash = calculate_at_hash(access_token);
    let c_hash = authorization_code.map(calculate_c_hash);
    let id_token = issue_id_token(
        state.upstream.issuer(),
        client_id,
        subject,
        nonce,
        Some(&at_hash),
        c_hash.as_deref(),
        i64::try_from(state.upstream.config.access_token_expires_in)
            .map_err(|_| OauthError::ServerError)?,
        Value::Object(user_claims),
        &state.upstream.keys,
    )
    .map_err(|_| OauthError::ServerError)?;

    Ok(Some(id_token))
}

fn current_timestamp_usize() -> Result<usize, OauthError> {
    usize::try_from(u64::try_from(Utc::now().timestamp()).map_err(|_| OauthError::ServerError)?)
        .map_err(|_| OauthError::ServerError)
}

fn expiry_timestamp_usize(ttl_seconds: u64) -> Result<usize, OauthError> {
    let ttl_seconds = i64::try_from(ttl_seconds).map_err(|_| OauthError::ServerError)?;
    usize::try_from(
        u64::try_from((Utc::now() + Duration::seconds(ttl_seconds)).timestamp())
            .map_err(|_| OauthError::ServerError)?,
    )
    .map_err(|_| OauthError::ServerError)
}
