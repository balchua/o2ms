#![allow(clippy::missing_errors_doc)]

use axum::{
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use oauth2_test_server::{
    error::OauthError,
    handlers::{
        device::{device_code, device_token, DeviceCodeRequest},
        discovery::{jwks, well_known_openid_configuration},
        error::error_page,
        introspect::introspect,
        register::{get_client, register_client},
        revoke::revoke,
        userinfo::userinfo,
    },
};
use std::collections::{BTreeMap, HashMap};

use crate::app::state::WrapperState;

pub async fn discovery(State(state): State<WrapperState>) -> Response {
    well_known_openid_configuration(State(state.upstream.clone()))
        .await
        .into_response()
}

pub async fn jwks_doc(State(state): State<WrapperState>) -> Response {
    jwks(State(state.upstream.clone())).await.into_response()
}

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

pub async fn userinfo_proxy(
    State(state): State<WrapperState>,
    headers: HeaderMap,
) -> Result<Response, OauthError> {
    userinfo(headers, State(state.upstream.clone()))
        .await
        .map(IntoResponse::into_response)
}

pub async fn device_code_proxy(
    State(state): State<WrapperState>,
    Form(form): Form<DeviceCodeRequest>,
) -> Result<Response, OauthError> {
    device_code(State(state.upstream.clone()), Form(form))
        .await
        .map(IntoResponse::into_response)
}

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