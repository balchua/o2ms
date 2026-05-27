# OAuth2 Mock Server

A standalone Rust OAuth/OIDC mock server for local testing with Spring Boot and other clients.

The repository is structured as a workspace:

- `crate/` contains the Rust application
- `examples/springboot-v4-resource-server/` contains a sample Spring Boot resource server
- `configs/` holds example config shapes that we will formalize in later tasks
- `docs/` contains setup and design documentation


## Quick design

The wrapper owns the outer server process and HTTP router.

```text
client
  -> wrapper router (`crate/src/http/router.rs`)
    -> local helper endpoints such as `/health`
    -> wrapper-owned `/token` with custom claim injection
    -> upstream OAuth/OIDC router from `oauth2-test-server`
      -> discovery, jwks, register, authorize, revoke, userinfo, device flow
```

The code is organized into:

- `app/` - startup and wrapper state
- `config/` - config model, default loading, validation
- `upstream/` - adapter to `oauth2-test-server` 
- `registry/` - client and user lookup/seeding
- `claims/` - claim merge, protection, and JWT issuance helpers
- `http/` - wrapper router and helper endpoints

## Running the Rust mock server

From the repository root:

```bash
cargo run -p oauth2-mock-server
```

To increase logging detail during troubleshooting:

```bash
RUST_LOG=oauth2_mock_server=debug cargo run -p oauth2-mock-server
```

Default local endpoints:

- `http://127.0.0.1:8090/health`
- `http://127.0.0.1:8090/.well-known/openid-configuration`
- `http://127.0.0.1:8090/.well-known/jwks.json`
- `http://127.0.0.1:8090/register`
- `http://127.0.0.1:8090/authorize`
- `http://127.0.0.1:8090/token`

## Running tests

From the repository root:

```bash
cargo test
```

## Running the linter

Clippy policy is defined in the project itself via the workspace `Cargo.toml`, and a cargo alias is provided for convenience.

From the repository root:

```bash
cargo lint
```

## Example Spring Boot app

The example app is under:

```text
examples/springboot-v4-resource-server/
```

Its intended role is to validate JWTs from the mock server via issuer discovery and JWKS.

This example now uses Maven with Spring Boot 4.0.6 and validates JWTs from the mock server via issuer discovery and JWKS.

Start it with:

```bash
cd examples/springboot-v4-resource-server
MOCK_ISSUER_URI=http://127.0.0.1:8090 mvn spring-boot:run
```

Or from the repository root:

```bash
MOCK_ISSUER_URI=http://127.0.0.1:8090 \
  mvn -f examples/springboot-v4-resource-server/pom.xml spring-boot:run
```

It listens on `http://127.0.0.1:8081`. For the browser flow, open `http://127.0.0.1:8081/login/me` and Spring Boot will redirect to the mock server picker before returning to the protected page. The fuller setup flow is documented in [docs/springboot.md](docs/springboot.md).

## Config examples

Example config files live under `configs/`, and the app now loads YAML from either `O2MS_CONFIG`, `configs/mock-server.yaml`, or built-in defaults.

See the documentation index for setup details:

- [docs/README.md](docs/README.md)
- [docs/api.md](docs/api.md)
- [docs/configuration.md](docs/configuration.md)
- [docs/quick-design.md](docs/quick-design.md)
- [docs/springboot.md](docs/springboot.md)

The runtime settings and OAuth/issuer behavior are now driven by YAML plus nested environment overrides documented in `docs/configuration.md`.

Configured `clients:` are preloaded into the in-memory OAuth store at startup, so most applications do **not** need to call `/register` unless you explicitly want dynamic registration behavior.

Configured `users:` now affect runtime behavior too: the first enabled user becomes the default identity for authorization-style user flows.

Configured custom claims now flow into JWT access tokens for the wrapper-owned `/token` grants:

- `authorization_code`
- `refresh_token`
- `client_credentials`

Token responses can now be configured globally or per-client to emit:

- the standard JSON body
- extra headers such as `Authorization` or `X-Access-Token`
- or both at the same time

For local UX, the binary now also accepts CLI flags such as `--config`, `--bind-port`, `--bind-host`, `--issuer-base-url`, and `--log-level`.
