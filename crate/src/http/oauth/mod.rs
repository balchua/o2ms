#![allow(clippy::missing_errors_doc)]

mod auth;
mod authorize;
mod proxy;
mod token;

use axum::{
    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use oauth2_test_server::{error::OauthError, handlers::{authorize::AuthorizeQuery, token::TokenRequest}};

use crate::app::state::WrapperState;

pub use authorize::UserPickerForm;

pub use proxy::{
    device_code_proxy, device_token_proxy, discovery, error_page_proxy, get_client_proxy,
    introspect_proxy, jwks_doc, register_client_proxy, revoke_proxy, userinfo_proxy,
};

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

    match authorize::validate_authorize_request(&state, &params).await {
        Ok(context) => {
            let users = authorize::eligible_users_for_client(&state, &context.client);
            if users.is_empty() {
                return authorize::authorize_error_redirect(
                    "invalid_request",
                    params.state.as_deref(),
                    Some("no_enabled_users_available_for_client"),
                );
            }

            authorize::render_user_picker_page(&params, &users, None).into_response()
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

    let context = match authorize::validate_authorize_request(&state, &params).await {
        Ok(context) => context,
        Err(response) => return response,
    };
    let users = authorize::eligible_users_for_client(&state, &context.client);

    let Some(selected_user) = users
        .into_iter()
        .find(|user| user.user_id == form.selected_user_id)
    else {
        return authorize::render_user_picker_page(
            &params,
            &authorize::eligible_users_for_client(&state, &context.client),
            Some("Select a valid enabled user for this client."),
        )
        .into_response();
    };

    authorize::issue_authorization_code_response(&state, &params, &context, selected_user.sub.as_str()).await
}

pub async fn token_endpoint(
    State(state): State<WrapperState>,
    headers: HeaderMap,
    Form(form): Form<TokenRequest>,
) -> Result<Response, OauthError> {
    if let Err(response) = auth::validate_client_auth(&state, &headers, &form).await {
        return Ok(response);
    }
    match form.grant_type.as_str() {
        "authorization_code" => token::handle_authorization_code(state, form).await,
        "refresh_token" => token::handle_refresh_token(state, form).await,
        "client_credentials" => token::handle_client_credentials(state, form).await,
        _ => Err(OauthError::UnsupportedGrantType),
    }
}