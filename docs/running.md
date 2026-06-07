# Running the server

This guide covers how to build and run the OAuth2 mock server for local development and testing.

## Prerequisites

- **Rust toolchain** (edition 2021). Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Java 21+** and **Maven** — only needed if running the Spring Boot examples.

## Building

From the repository root:

```bash
cargo build -p o2ms
```

Or build with optimizations:

```bash
cargo build --release -p o2ms
```

## Running

### Default mode (OAuth-only)

```bash
cargo run -p o2ms
```

This starts the server on `http://127.0.0.1:8090` with built-in defaults. No config file is required.

### With a config file

```bash
cargo run -p o2ms -- --config configs/mock-server.yaml
```

### With environment variables

```bash
O2MS_SERVER__BIND_PORT=9191 \
O2MS_ISSUER__BASE_URL=http://127.0.0.1:9191 \
  cargo run -p o2ms
```

### With gateway enabled

```bash
O2MS_GATEWAY__ENABLED=true \
  cargo run -p o2ms
```

### With verbose logging

```bash
RUST_LOG=o2ms=debug cargo run -p o2ms
```

## CLI flags

All flags are optional. When provided, they override YAML config and environment variables.

| Flag | Type | Description |
|---|---|---|
| `--config <path>` | string | Path to a YAML config file |
| `--bind-host <host>` | string | Override `server.bind_host` (default: `127.0.0.1`) |
| `--bind-port <port>` | u16 | Override `server.bind_port` (default: `8090`) |
| `--issuer-base-url <url>` | string | Override `issuer.base_url` |
| `--log-level <level>` | string | Override `server.log_level` (e.g. `debug`, `info`) |
| `--health-endpoint-enabled <bool>` | `true`/`false` | Override `server.health_endpoint_enabled` |
| `--runtime-client-registration-enabled <bool>` | `true`/`false` | Override `server.runtime_client_registration_enabled` |
| `-h`, `--help` | — | Show help text |

Flags can be passed as `--flag value` or `--flag=value`:

```bash
cargo run -p o2ms -- \
  --config configs/mock-server.yaml \
  --bind-port 9191 \
  --issuer-base-url http://127.0.0.1:9191 \
  --log-level debug
```

## Config loading order

The server resolves configuration in this order (later sources override earlier ones):

1. Built-in defaults
2. YAML file (from `O2MS_CONFIG` env var, or `--config` flag, or `configs/mock-server.yaml` if it exists)
3. Environment variables (`O2MS_*`)
4. CLI flags

## Default endpoints

When started with defaults, the server exposes:

| Endpoint | Purpose |
|---|---|
| `http://127.0.0.1:8090/health` | Health check |
| `http://127.0.0.1:8090/.well-known/openid-configuration` | OIDC discovery |
| `http://127.0.0.1:8090/.well-known/jwks.json` | JWKS public keys |
| `http://127.0.0.1:8090/authorize` | OAuth authorization |
| `http://127.0.0.1:8090/token` | OAuth token endpoint |
| `http://127.0.0.1:8090/register` | Dynamic client registration |
| `http://127.0.0.1:8090/introspect` | Token introspection |
| `http://127.0.0.1:8090/revoke` | Token revocation |
| `http://127.0.0.1:8090/userinfo` | UserInfo endpoint |

## Running tests

```bash
cargo test
```

## Running the linter

```bash
cargo lint
```

## Config files

Pre-built config files are in `configs/`:

- `configs/mock-server.yaml` — general-purpose defaults
- `configs/mock-server.springboot.yaml` — Spring Boot-oriented config with user picker and token response headers

## Example applications

- **Spring Boot resource server** — see [springboot.md](springboot.md)
- **Gateway client** — see [gateway.md](gateway.md)
- **Java client through gateway** — see the gateway section in [springboot.md](springboot.md#gateway-client-example)