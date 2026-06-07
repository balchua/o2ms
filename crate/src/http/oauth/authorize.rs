use std::fmt::Write;

use axum::{
    response::{Html, IntoResponse, Redirect, Response},
};
use oauth2_test_server::{
    crypto::generate_code,
    handlers::authorize::AuthorizeQuery,
    models::{AuthorizationCode, Client},
};
use chrono::{Duration, Utc};

use crate::{
    app::state::WrapperState,
    registry::clients::find_enabled_client_by_id,
    registry::users::{enabled_users, linked_enabled_users},
};

#[derive(Clone)]
pub struct AuthorizeContext {
    pub client: Client,
    pub redirect_uri: String,
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
    #[must_use]
    pub fn to_authorize_query(&self) -> AuthorizeQuery {
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

pub async fn validate_authorize_request(
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

pub fn eligible_users_for_client<'a>(
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

pub async fn issue_authorization_code_response(
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

pub fn authorize_error_redirect(
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

pub fn render_user_picker_page(
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