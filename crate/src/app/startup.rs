use std::net::SocketAddr;

use tokio::{net::TcpListener, task::JoinHandle};

use crate::{
    app::state::WrapperState,
    config::{
        loader::{load, load_from_sources},
        model::AppConfig,
    },
    error::AppError,
    http::{admin, router::build_router},
    registry::clients::seed_clients,
    registry::users::default_user,
    upstream::adapter::build_upstream_state,
};

pub struct RunningServer {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl RunningServer {
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub async fn shutdown(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// Start the wrapper server on the configured bind address.
///
/// # Errors
///
/// Returns an error when config validation fails or the TCP listener cannot be
/// created for the requested address.
pub async fn spawn(config: AppConfig) -> Result<RunningServer, AppError> {
    tracing::debug!("loading application config");
    let config = load(Some(config))?;
    tracing::info!(
        bind_host = %config.server.bind_host,
        bind_port = config.server.bind_port,
        startup_mode = ?config.server.startup_mode,
        "binding oauth2 mock server"
    );
    if let Some(seed) = config.server.deterministic_seed {
        tracing::warn!(
            deterministic_seed = seed,
            "deterministic_seed is configured but not yet wired into upstream key/token generation"
        );
    }
    if let Some(user) = default_user(&config) {
        tracing::info!(
            user_id = %user.user_id,
            sub = %user.sub,
            "using first enabled configured user as the default authorization-flow user"
        );
    } else {
        tracing::info!("no enabled configured users found; using upstream fallback default user");
    }
    let listener = TcpListener::bind((config.server.bind_host.as_str(), config.server.bind_port))
        .await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "oauth2 mock server bound successfully");

    let upstream_state = build_upstream_state(&config, addr.port());
    let seeded_clients = seed_clients(&config, &upstream_state).await?;
    tracing::info!(seeded_clients, "preloaded configured clients into oauth store");
    let wrapper_state = WrapperState::new(config.clone(), upstream_state);
    let router = build_router(&config, wrapper_state);

    let handle = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            tracing::error!(%error, "wrapper server failed");
        }
    });

    Ok(RunningServer { addr, handle })
}

/// Run the wrapper server until a Ctrl+C signal is received.
///
/// # Errors
///
/// Returns an error when startup validation fails, binding the listener fails,
/// or waiting for the shutdown signal fails.
pub async fn run(config: AppConfig) -> Result<(), AppError> {
    let runtime_client_registration_enabled = config.server.runtime_client_registration_enabled;
    let health_endpoint_enabled = config.server.health_endpoint_enabled;
    let list_clients_endpoint_enabled = config.admin.list_clients_endpoint_enabled;
    let reset_endpoint_enabled = config.admin.reset_endpoint_enabled;
    let config_endpoint_enabled = config.admin.config_endpoint_enabled;
    let admin_endpoints_requested = admin::endpoints_enabled(&config);
    let admin_endpoints_enabled = admin_endpoints_requested
        && admin::bind_host_allows_admin_endpoints(config.server.bind_host.as_str());
    let require_state = config.oauth.require_state;
    let signing_algorithm = config.oauth.signing_algorithm.clone();
    let server = spawn(config).await?;
    let base_url = server.base_url();
    tracing::info!(%base_url, "oauth2 mock server running");
    if health_endpoint_enabled {
        tracing::info!(health = %format!("{base_url}/health"), "health endpoint");
    } else {
        tracing::info!("health endpoint disabled");
    }
    tracing::info!(
        discovery = %format!("{base_url}/.well-known/openid-configuration"),
        "discovery endpoint"
    );
    tracing::info!(jwks = %format!("{base_url}/.well-known/jwks.json"), "jwks endpoint");
    if runtime_client_registration_enabled {
        tracing::info!(register = %format!("{base_url}/register"), "register endpoint");
    } else {
        tracing::info!("runtime client registration disabled");
    }
    if admin_endpoints_enabled {
        if list_clients_endpoint_enabled {
            tracing::info!(admin_clients = %format!("{base_url}/admin/clients"), "admin clients endpoint");
        }
        if reset_endpoint_enabled {
            tracing::info!(admin_reset = %format!("{base_url}/admin/reset"), "admin reset endpoint");
        }
        if config_endpoint_enabled {
            tracing::info!(admin_config = %format!("{base_url}/admin/config"), "admin config endpoint");
        }
    } else if admin_endpoints_requested {
        tracing::warn!(
            bind_host = %server.addr().ip(),
            "admin endpoints were requested but not mounted because admin routes are limited to loopback bind hosts"
        );
    }
    tracing::info!(authorize = %format!("{base_url}/authorize"), "authorize endpoint");
    tracing::info!(token = %format!("{base_url}/token"), "token endpoint");
    tracing::info!(
        runtime_client_registration_enabled,
        health_endpoint_enabled,
        admin_endpoints_enabled,
        require_state,
        signing_algorithm = %signing_algorithm,
        "runtime settings applied"
    );

    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown signal received");
    server.shutdown().await;
    tracing::info!("oauth2 mock server stopped");
    Ok(())
}

/// Run the wrapper server using YAML/default config sources.
///
/// # Errors
///
/// Returns an error when config loading, validation, binding, or shutdown
/// signal handling fails.
pub async fn run_from_sources() -> Result<(), AppError> {
    let config = load_from_sources()?;
    run(config).await
}
