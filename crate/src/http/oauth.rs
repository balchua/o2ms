use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use oauth2_test_server::{
    crypto::generate_code,
    error::OauthError,
    handlers::{
        authorize::AuthorizeQuery,
        device::{device_code, device_token, DeviceCodeRequest},
        discovery::{jwks, well_known_openid_configuration},
        error::error_page,
        introspect::introspect,
        register::{get_client, register_client},
        revoke::revoke,
        token::TokenRequest,
        userinfo::userinfo,
    },
    models::{AuthorizationCode, Client, Token},
};
use chrono::{Duration, Utc};

use crate::{
    app::state::WrapperState,
    claims::issue::{issue_access_token, issue_optional_id_token},
    registry::clients::find_enabled_client_by_id,
    registry::users::{enabled_users, linked_enabled_users},
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
    if !state.config.oauth.authorization_user_picker_enabled || params.response_type != "code" {
        return oauth2_test_server::handlers::authorize::authorize(
            State(state.upstream.clone()),
            Query(params),
        )
        .await
        .into_response();
    }

    match validate_authorize_request(&state, &params).await {
        Ok(context) => {
            let users = eligible_users_for_client(&state, &context.client);
            if users.is_empty() {
                return authorize_error_redirect(
                    "invalid_request",
                    params.state.as_deref(),
                    Some("no_enabled_users_available_for_client"),
                );
            }

            render_user_picker_page(&params, &users, None).into_response()
        }
        Err(response) => response,
    }
}

pub async fn authorize_submit(
    State(state): State<WrapperState>,
    Form(form): Form<UserPickerForm>,
) -> Response {
    let params = form.to_authorize_query();

    if !state.config.oauth.authorization_user_picker_enabled || params.response_type != "code" {
        return StatusCode::NOT_FOUND.into_response();
    }

    let context = match validate_authorize_request(&state, &params).await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let users = eligible_users_for_client(&state, &context.client);

    let Some(selected_user) = users
        .into_iter()
        .find(|user| user.user_id == form.selected_user_id)
    else {
        return render_user_picker_page(
            &params,
            &eligible_users_for_client(&state, &context.client),
            Some("Select a valid enabled user for this client."),
        )
        .into_response();
    };

    issue_authorization_code_response(&state, &params, &context, selected_user.sub.as_str()).await
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

#[derive(Clone)]
struct AuthorizeContext {
    client: Client,
    redirect_uri: String,
}

#[derive(serde::Deserialize)]
pub struct UserPickerForm {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub response_mode: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub nonce: Option<String>,
    pub prompt: Option<String>,
    pub max_age: Option<String>,
    pub claims: Option<String>,
    pub ui_locales: Option<String>,
    pub selected_user_id: String,
}

impl UserPickerForm {
    fn to_authorize_query(&self) -> AuthorizeQuery {
        AuthorizeQuery {
            response_type: self.response_type.clone(),
            client_id: self.client_id.clone(),
            redirect_uri: self.redirect_uri.clone(),
            scope: self.scope.clone(),
            state: self.state.clone(),
            response_mode: self.response_mode.clone(),
            code_challenge: self.code_challenge.clone(),
            code_challenge_method: self.code_challenge_method.clone(),
            nonce: self.nonce.clone(),
            prompt: self.prompt.clone(),
            max_age: self.max_age.clone(),
            claims: self.claims.clone(),
            ui_locales: self.ui_locales.clone(),
        }
    }
}

async fn validate_authorize_request(
    state: &WrapperState,
    params: &AuthorizeQuery,
) -> Result<AuthorizeContext, Response> {
    let Some(client) = state.upstream.store.get_client(&params.client_id).await else {
        return Err(authorize_error_redirect(
            "invalid_client",
            params.state.as_deref(),
            None,
        ));
    };

    if state.upstream.config.require_state && params.state.is_none() {
        return Err(
            authorize_error_redirect(
                "invalid_request",
                params.state.as_deref(),
                Some("state_parameter_required"),
            ),
        );
    }

    let supported_response_types = [
        "code",
        "token",
        "id_token",
        "code token",
        "code id_token",
        "token id_token",
        "code token id_token",
    ];
    if !supported_response_types.contains(&params.response_type.as_str()) {
        return Err(authorize_error_redirect(
            "unsupported_response_type",
            params.state.as_deref(),
            None,
        ));
    }

    if let Some(prompt) = &params.prompt {
        if let Some(parsed_prompt) = oauth2_test_server::handlers::authorize::Prompt::from_str(prompt) {
            if parsed_prompt == oauth2_test_server::handlers::authorize::Prompt::None {
                return Err(authorize_error_redirect(
                    "invalid_request",
                    params.state.as_deref(),
                    Some("prompt=none_requires_no_existing_session"),
                ));
            }
        } else {
            return Err(authorize_error_redirect(
                "invalid_request",
                params.state.as_deref(),
                Some("invalid_prompt_value"),
            ));
        }
    }

    if let Some(max_age) = &params.max_age
        && max_age.parse::<i64>().is_err()
    {
        return Err(authorize_error_redirect(
            "invalid_request",
            params.state.as_deref(),
            Some("max_age_must_be_an_integer"),
        ));
    }

    if let Some(claims) = &params.claims
        && serde_json::from_str::<serde_json::Value>(claims).is_err()
    {
        return Err(authorize_error_redirect(
            "invalid_request",
            params.state.as_deref(),
            Some("invalid_claims_parameter"),
        ));
    }

    let redirect_uri = match &params.redirect_uri {
        Some(uri) => {
            if !client.redirect_uris.contains(uri) {
                return Err(authorize_error_redirect(
                    "invalid_request",
                    params.state.as_deref(),
                    Some("invalid_redirect_uri"),
                ));
            }
            uri.clone()
        }
        None => match client.redirect_uris.first() {
            Some(uri) => uri.clone(),
            None => {
                return Err(authorize_error_redirect(
                    "invalid_request",
                    params.state.as_deref(),
                    Some("no_redirect_uri"),
                ));
            }
        }
    };

    Ok(AuthorizeContext {
        client,
        redirect_uri,
    })
}

fn eligible_users_for_client<'a>(
    state: &'a WrapperState,
    client: &Client,
) -> Vec<&'a crate::config::model::UserConfig> {
    if let Some(configured_client) = find_enabled_client_by_id(&state.config, &client.client_id) {
        if configured_client.linked_users.is_empty() {
            enabled_users(&state.config)
        } else {
            linked_enabled_users(&state.config, &configured_client.linked_users)
        }
    } else {
        enabled_users(&state.config)
    }
}

async fn issue_authorization_code_response(
    state: &WrapperState,
    params: &AuthorizeQuery,
    context: &AuthorizeContext,
    selected_subject: &str,
) -> Response {
    use std::collections::HashSet;

    let code = generate_code();
    let requested_scopes: HashSet<String> = params
        .scope
        .clone()
        .unwrap_or_default()
        .split_whitespace()
        .map(ToString::to_string)
        .collect();
    let registered_scopes: HashSet<String> = context
        .client
        .scope
        .split_whitespace()
        .map(ToString::to_string)
        .collect();
    let granted_scopes: Vec<String> = requested_scopes
        .intersection(&registered_scopes)
        .cloned()
        .collect();
    let final_scope = granted_scopes.join(" ");

    let auth_code = AuthorizationCode {
        code: code.clone(),
        client_id: params.client_id.clone(),
        redirect_uri: context.redirect_uri.clone(),
        scope: final_scope,
        expires_at: Utc::now()
            + Duration::seconds(
                i64::try_from(state.upstream.config.authorization_code_expires_in)
                    .map_or(600, std::convert::identity),
            ),
        code_challenge: params.code_challenge.clone(),
        code_challenge_method: params.code_challenge_method.clone(),
        user_id: selected_subject.to_string(),
        nonce: params.nonce.clone(),
        state: params.state.clone(),
    };

    state.upstream.store.insert_code(code.clone(), auth_code).await;

    let response_mode = params.response_mode.as_deref().unwrap_or("query");
    let state_param = params.state.as_deref().unwrap_or("");

    match response_mode {
        "form_post" => Html(format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Redirecting</title></head>
<body>
<form id="redirect-form" method="POST" action="{redirect_uri}">
<input type="hidden" name="code" value="{code}"/>
<input type="hidden" name="state" value="{state_param}"/>
</form>
<script>document.getElementById('redirect-form').submit();</script>
</body>
</html>"#,
            redirect_uri = escape_html(context.redirect_uri.as_str()),
            code = escape_html(code.as_str()),
            state_param = escape_html(state_param),
        ))
        .into_response(),
        "fragment" => Redirect::to(&format!(
            "{}#code={}&state={}",
            context.redirect_uri, code, state_param
        ))
        .into_response(),
        _ => Redirect::to(&format!(
            "{}?code={}&state={}",
            context.redirect_uri, code, state_param
        ))
        .into_response(),
    }
}

fn authorize_error_redirect(
    error: &str,
    state: Option<&str>,
    description: Option<&str>,
) -> Response {
    let mut target = format!("/error?error={error}&state={}", state.unwrap_or(""));
    if let Some(description) = description {
        target.push_str("&error_description=");
        target.push_str(description);
    }

    Redirect::to(target.as_str()).into_response()
}

fn render_user_picker_page(
    params: &AuthorizeQuery,
    users: &[&crate::config::model::UserConfig],
    error_message: Option<&str>,
) -> Html<String> {
    let mut options = String::new();
    for user in users {
        let _ = write!(
            options,
            r#"<option value="{user_id}">{label}</option>"#,
            user_id = escape_html(user.user_id.as_str()),
            label = escape_html(
                format!("{} ({})", user.display_name, user.sub).as_str()
            ),
        );
    }

    let error_markup = error_message.map_or_else(String::new, |message| {
        format!(
            r#"<p style="color: #b91c1c; margin-bottom: 1rem;">{}</p>"#,
            escape_html(message)
        )
    });

    let mut hidden_fields = String::new();
    push_hidden(&mut hidden_fields, "response_type", Some(params.response_type.as_str()));
    push_hidden(&mut hidden_fields, "client_id", Some(params.client_id.as_str()));
    push_hidden(&mut hidden_fields, "redirect_uri", params.redirect_uri.as_deref());
    push_hidden(&mut hidden_fields, "scope", params.scope.as_deref());
    push_hidden(&mut hidden_fields, "state", params.state.as_deref());
    push_hidden(&mut hidden_fields, "response_mode", params.response_mode.as_deref());
    push_hidden(&mut hidden_fields, "code_challenge", params.code_challenge.as_deref());
    push_hidden(
        &mut hidden_fields,
        "code_challenge_method",
        params.code_challenge_method.as_deref(),
    );
    push_hidden(&mut hidden_fields, "nonce", params.nonce.as_deref());
    push_hidden(&mut hidden_fields, "prompt", params.prompt.as_deref());
    push_hidden(&mut hidden_fields, "max_age", params.max_age.as_deref());
    push_hidden(&mut hidden_fields, "claims", params.claims.as_deref());
    push_hidden(&mut hidden_fields, "ui_locales", params.ui_locales.as_deref());

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>Select test user</title>
</head>
<body style="font-family: sans-serif; max-width: 640px; margin: 3rem auto; padding: 0 1rem;">
  <h1>Select a test user</h1>
  <p>Client: <strong>{client_id}</strong></p>
  <p>Scope: <code>{scope}</code></p>
  {error_markup}
  <form method="post" action="/authorize">
    {hidden_fields}
    <label for="selected_user_id">User ID</label><br/>
    <input id="selected_user_id" name="selected_user_id" list="yaml-users" required style="min-width: 24rem; padding: 0.4rem; margin: 0.5rem 0 1rem;"/><br/>
    <datalist id="yaml-users">
      {options}
    </datalist>
    <button type="submit" style="padding: 0.6rem 1rem;">Continue</button>
  </form>
</body>
</html>"#,
        client_id = escape_html(params.client_id.as_str()),
        scope = escape_html(params.scope.as_deref().unwrap_or("")),
        error_markup = error_markup,
        hidden_fields = hidden_fields,
        options = options,
    ))
}

fn push_hidden(target: &mut String, field: &str, value: Option<&str>) {
    if let Some(value) = value {
        let _ = write!(
            target,
            r#"<input type="hidden" name="{field}" value="{value}"/>"#,
            field = escape_html(field),
            value = escape_html(value),
        );
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
