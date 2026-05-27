use oauth2_mock_server::{
    AppConfig, ClientConfig, HeaderValueFormat, ServerConfig, TokenField, TokenHeaderConfig,
    TokenResponseConfig, spawn,
};
use serde_json::Value;

#[tokio::test]
async fn server_exposes_health_and_discovery_endpoints() -> Result<(), Box<dyn std::error::Error>>
{
    let server = spawn(AppConfig {
        server: ServerConfig {
            bind_host: "127.0.0.1".to_string(),
            bind_port: 0,
            ..ServerConfig::default()
        },
        ..AppConfig::default()
    })
    .await?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let health = client
        .get(format!("{}/health", server.base_url()))
        .send()
        .await?;
    assert!(health.status().is_success());
    assert_eq!(health.text().await?, "ok");

    let discovery = client
        .get(format!(
            "{}/.well-known/openid-configuration",
            server.base_url()
        ))
        .send()
        .await?;
    assert!(discovery.status().is_success());
    let discovery_json: serde_json::Value = discovery.json().await?;
    assert_eq!(
        discovery_json["issuer"].as_str(),
        Some(server.base_url().as_str())
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn token_endpoint_can_emit_header_only_response() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.token_response = TokenResponseConfig {
        emit_json_body: false,
        emit_headers: vec![TokenHeaderConfig::default()],
    };
    config.clients = vec![ClientConfig {
        client_id: "header-only-client".to_string(),
        client_name: "Header Only Client".to_string(),
        ..ClientConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::new();

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=header-only-client&scope=openid")
        .send()
        .await?;

    assert!(token.status().is_success());
    let authorization_header = token
        .headers()
        .get(reqwest::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or("authorization header missing")?;
    assert!(authorization_header.starts_with("Bearer "));
    assert!(token.text().await?.is_empty());

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn client_override_can_emit_custom_token_header() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.token_response = TokenResponseConfig {
        emit_json_body: true,
        emit_headers: Vec::new(),
    };
    config.clients = vec![ClientConfig {
        client_id: "override-client".to_string(),
        client_name: "Override Client".to_string(),
        token_response_override: Some(TokenResponseConfig {
            emit_json_body: true,
            emit_headers: vec![TokenHeaderConfig {
                header_name: "X-Access-Token".to_string(),
                token_field: TokenField::AccessToken,
                value_format: HeaderValueFormat::Raw,
            }],
        }),
        ..ClientConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::new();

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=override-client&scope=openid")
        .send()
        .await?;

    assert!(token.status().is_success());
    let header_value = token
        .headers()
        .get("X-Access-Token")
        .and_then(|value| value.to_str().ok())
        .ok_or("custom token header missing")?
        .to_string();
    let token_json: serde_json::Value = token.json().await?;
    assert_eq!(token_json["access_token"].as_str(), Some(header_value.as_str()));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn runtime_registered_clients_appear_in_admin_listing(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.admin.list_clients_endpoint_enabled = true;
    config.clients = vec![ClientConfig {
        client_id: "seeded-client".to_string(),
        client_name: "Seeded Client".to_string(),
        ..ClientConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::new();

    let registration = client
        .post(format!("{}/register", server.base_url()))
        .json(&serde_json::json!({
            "redirect_uris": ["http://localhost:8080/callback"],
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "scope": "openid"
        }))
        .send()
        .await?;

    assert_eq!(registration.status(), reqwest::StatusCode::CREATED);
    let runtime_client_id = registration
        .json::<serde_json::Value>()
        .await?["client_id"]
        .as_str()
        .ok_or("runtime client id missing")?
        .to_string();

    let listed_clients = client
        .get(format!("{}/admin/clients", server.base_url()))
        .send()
        .await?;

    assert!(listed_clients.status().is_success());
    let listed_clients_json = listed_clients.json::<serde_json::Value>().await?;
    assert!(listed_clients_json
        .as_array()
        .ok_or("admin clients response must be an array")?
        .iter()
        .any(|entry| entry["client_id"].as_str() == Some("seeded-client")
            && entry["source"].as_str() == Some("preloaded")));
    assert!(listed_clients_json
        .as_array()
        .ok_or("admin clients response must be an array")?
        .iter()
        .any(|entry| entry["client_id"].as_str() == Some(runtime_client_id.as_str())
            && entry["source"].as_str() == Some("runtime")));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn admin_reset_clears_runtime_state_and_reseeds_configured_clients(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.admin.list_clients_endpoint_enabled = true;
    config.admin.reset_endpoint_enabled = true;
    config.clients = vec![ClientConfig {
        client_id: "seeded-client".to_string(),
        client_name: "Seeded Client".to_string(),
        ..ClientConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::new();

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=seeded-client&scope=openid")
        .send()
        .await?;
    let access_token = token
        .json::<serde_json::Value>()
        .await?["access_token"]
        .as_str()
        .ok_or("access token missing")?
        .to_string();

    let registration = client
        .post(format!("{}/register", server.base_url()))
        .json(&serde_json::json!({
            "redirect_uris": ["http://localhost:8080/callback"],
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "scope": "openid"
        }))
        .send()
        .await?;
    let runtime_client_id = registration
        .json::<serde_json::Value>()
        .await?["client_id"]
        .as_str()
        .ok_or("runtime client id missing")?
        .to_string();

    let reset = client
        .post(format!("{}/admin/reset", server.base_url()))
        .send()
        .await?;

    assert!(reset.status().is_success());
    let reset_json = reset.json::<serde_json::Value>().await?;
    assert_eq!(reset_json["reseeded_clients"].as_u64(), Some(1));
    assert_eq!(reset_json["clients_after"].as_u64(), Some(1));

    let listed_clients = client
        .get(format!("{}/admin/clients", server.base_url()))
        .send()
        .await?;
    let listed_clients_json = listed_clients.json::<serde_json::Value>().await?;
    let listed_array = listed_clients_json
        .as_array()
        .ok_or("admin clients response must be an array")?;
    assert!(listed_array
        .iter()
        .any(|entry| entry["client_id"].as_str() == Some("seeded-client")));
    assert!(!listed_array
        .iter()
        .any(|entry| entry["client_id"].as_str() == Some(runtime_client_id.as_str())));

    let userinfo = client
        .get(format!("{}/userinfo", server.base_url()))
        .bearer_auth(access_token)
        .send()
        .await?;
    assert_eq!(userinfo.status(), reqwest::StatusCode::UNAUTHORIZED);

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn preloaded_client_can_use_token_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.clients = vec![oauth2_mock_server::ClientConfig {
        client_id: "seeded-client".to_string(),
        client_name: "Seeded Client".to_string(),
        allowed_scopes: vec!["openid".to_string(), "profile".to_string()],
        default_scopes: vec!["openid".to_string()],
        ..oauth2_mock_server::ClientConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=seeded-client&scope=openid")
        .send()
        .await?;

    assert!(token.status().is_success());
    let token_json: serde_json::Value = token.json().await?;
    assert!(token_json["access_token"].as_str().is_some());
    assert_eq!(token_json["scope"].as_str(), Some("openid"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn configured_claims_are_embedded_in_issued_access_tokens(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.clients = vec![oauth2_mock_server::ClientConfig {
        client_id: "claims-client".to_string(),
        client_name: "Claims Client".to_string(),
        allowed_scopes: vec!["openid".to_string()],
        default_scopes: vec!["openid".to_string()],
        claims_template_refs: vec!["shared".to_string()],
        custom_claims: std::collections::BTreeMap::from([(
            "tenant".to_string(),
            serde_json::json!("sandbox"),
        )]),
        ..oauth2_mock_server::ClientConfig::default()
    }];
    config.claims_templates = std::collections::BTreeMap::from([(
        "shared".to_string(),
        std::collections::BTreeMap::from([(
            "authorizations".to_string(),
            serde_json::json!([]),
        )]),
    )]);

    let server = spawn(config).await?;
    let client = reqwest::Client::new();

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials&client_id=claims-client&scope=openid")
        .send()
        .await?;

    assert!(token.status().is_success());
    let token_json: serde_json::Value = token.json().await?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or("access token missing")?;
    let claims = decode_jwt_payload(access_token)?;

    assert_eq!(claims["tenant"], serde_json::json!("sandbox"));
    assert_eq!(claims["authorizations"], serde_json::json!([]));
    assert_eq!(claims["sub"], serde_json::json!("client"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn configured_user_becomes_default_authorization_flow_subject(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.oauth.require_state = false;
    config.clients = vec![oauth2_mock_server::ClientConfig {
        client_id: "auth-client".to_string(),
        client_name: "Auth Client".to_string(),
        redirect_uris: vec!["http://localhost:8080/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        allowed_scopes: vec!["openid".to_string()],
        default_scopes: vec!["openid".to_string()],
        linked_users: vec!["demo-user".to_string()],
        ..oauth2_mock_server::ClientConfig::default()
    }];
    config.users = vec![oauth2_mock_server::UserConfig {
        user_id: "demo-user".to_string(),
        sub: "demo-subject".to_string(),
        username: "demo".to_string(),
        email: "demo@example.com".to_string(),
        display_name: "Demo User".to_string(),
        ..oauth2_mock_server::UserConfig::default()
    }];

    let server = spawn(config).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let authorize = client
        .get(format!("{}/authorize", server.base_url()))
        .query(&[
            ("response_type", "code"),
            ("client_id", "auth-client"),
            ("redirect_uri", "http://localhost:8080/callback"),
            ("scope", "openid"),
        ])
        .send()
        .await?;

    assert!(authorize.status().is_redirection());
    let location = authorize
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or("authorize redirect location missing")?;
    let code = location
        .split('?')
        .nth(1)
        .and_then(|query| query.split('&').find(|part| part.starts_with("code=")))
        .and_then(|part| part.split('=').nth(1))
        .ok_or("authorization code missing from redirect")?;

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=authorization_code&client_id=auth-client&code={code}"
        ))
        .send()
        .await?;

    assert!(token.status().is_success());
    let token_json: serde_json::Value = token.json().await?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or("access token missing")?;

    let userinfo = client
        .get(format!("{}/userinfo", server.base_url()))
        .bearer_auth(access_token)
        .send()
        .await?;

    assert!(userinfo.status().is_success());
    let userinfo_json: serde_json::Value = userinfo.json().await?;
    assert_eq!(userinfo_json["sub"].as_str(), Some("demo-subject"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn authorization_picker_lists_eligible_users_and_issues_selected_user_code(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = AppConfig::default();
    config.server.bind_port = 0;
    config.oauth.require_state = false;
    config.oauth.authorization_user_picker_enabled = true;
    config.clients = vec![oauth2_mock_server::ClientConfig {
        client_id: "picker-client".to_string(),
        client_name: "Picker Client".to_string(),
        redirect_uris: vec!["http://localhost:8080/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        allowed_scopes: vec!["openid".to_string()],
        default_scopes: vec!["openid".to_string()],
        linked_users: vec!["demo-user".to_string()],
        ..oauth2_mock_server::ClientConfig::default()
    }];
    config.users = vec![
        oauth2_mock_server::UserConfig {
            user_id: "demo-user".to_string(),
            sub: "demo-subject".to_string(),
            username: "demo".to_string(),
            email: "demo@example.com".to_string(),
            display_name: "Demo User".to_string(),
            ..oauth2_mock_server::UserConfig::default()
        },
        oauth2_mock_server::UserConfig {
            user_id: "support-user".to_string(),
            sub: "support-subject".to_string(),
            username: "support".to_string(),
            email: "support@example.com".to_string(),
            display_name: "Support User".to_string(),
            ..oauth2_mock_server::UserConfig::default()
        },
    ];

    let server = spawn(config).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let authorize = client
        .get(format!("{}/authorize", server.base_url()))
        .query(&[
            ("response_type", "code"),
            ("client_id", "picker-client"),
            ("redirect_uri", "http://localhost:8080/callback"),
            ("scope", "openid"),
        ])
        .send()
        .await?;

    assert_eq!(authorize.status(), reqwest::StatusCode::OK);
    let picker_html = authorize.text().await?;
    assert!(picker_html.contains("Select a test user"));
    assert!(picker_html.contains(r#"value="demo-user""#));
    assert!(!picker_html.contains(r#"value="support-user""#));

    let authorize_submit = client
        .post(format!("{}/authorize", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(
            "response_type=code&client_id=picker-client&redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback&scope=openid&selected_user_id=demo-user",
        )
        .send()
        .await?;

    assert!(authorize_submit.status().is_redirection());
    let location = authorize_submit
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or("authorize redirect location missing")?;
    let code = location
        .split('?')
        .nth(1)
        .and_then(|query| query.split('&').find(|part| part.starts_with("code=")))
        .and_then(|part| part.split('=').nth(1))
        .ok_or("authorization code missing from redirect")?;

    let token = client
        .post(format!("{}/token", server.base_url()))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=authorization_code&client_id=picker-client&code={code}"
        ))
        .send()
        .await?;

    assert!(token.status().is_success());
    let token_json: serde_json::Value = token.json().await?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or("access token missing")?;

    let userinfo = client
        .get(format!("{}/userinfo", server.base_url()))
        .bearer_auth(access_token)
        .send()
        .await?;

    assert!(userinfo.status().is_success());
    let userinfo_json: serde_json::Value = userinfo.json().await?;
    assert_eq!(userinfo_json["sub"].as_str(), Some("demo-subject"));

    server.shutdown().await;
    Ok(())
}

fn decode_jwt_payload(token: &str) -> Result<Value, Box<dyn std::error::Error>> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    let payload = token.split('.').nth(1).ok_or("jwt payload missing")?;
    let decoded = URL_SAFE_NO_PAD.decode(payload)?;
    Ok(serde_json::from_slice(&decoded)?)
}
