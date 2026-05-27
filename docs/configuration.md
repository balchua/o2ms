# Configuration guide

The mock server uses **YAML** for user-facing configuration.

## How configuration is loaded

The application resolves configuration in this order:

1. If `O2MS_CONFIG` is set, load that YAML file.
2. Otherwise, if `configs/mock-server.yaml` exists, load it.
3. Layer nested environment variables on top.
4. Otherwise, use built-in defaults for anything still unset.

The loader now uses the Rust [`config`](https://crates.io/crates/config) crate for YAML loading and environment layering.

## Environment variable configuration

Environment variables use the prefix `O2MS_` and `__` to separate nested keys.

Examples:

```bash
O2MS_SERVER__BIND_HOST=0.0.0.0
O2MS_SERVER__BIND_PORT=9191
O2MS_SERVER__LOG_LEVEL=debug
O2MS_SERVER__CORS_ALLOWED_ORIGINS=http://localhost:8080,http://localhost:3000
O2MS_SERVER__HEALTH_ENDPOINT_ENABLED=false
O2MS_SERVER__RUNTIME_CLIENT_REGISTRATION_ENABLED=false
O2MS_ISSUER__BASE_URL=http://127.0.0.1:9191
O2MS_OAUTH__REQUIRE_STATE=false
O2MS_OAUTH__ACCESS_TOKEN_TTL_SECONDS=120
```

Rules:

- use `__` between section and field names
- booleans and integers are parsed automatically
- comma-separated values are supported for `server.cors_allowed_origins`
- `O2MS_CONFIG` is the dedicated environment variable for selecting the YAML file path

## Start the server with a config file

```bash
O2MS_CONFIG=configs/mock-server.yaml cargo run -p oauth2-mock-server
```

Or run fully from environment variables:

```bash
O2MS_SERVER__BIND_PORT=9191 \
O2MS_ISSUER__BASE_URL=http://127.0.0.1:9191 \
cargo run -p oauth2-mock-server
```

## CLI flags

Common runtime overrides can also be supplied from the command line:

```bash
cargo run -p oauth2-mock-server -- \
  --config configs/mock-server.yaml \
  --bind-port 9191 \
  --issuer-base-url http://127.0.0.1:9191 \
  --log-level debug
```

Supported flags:

- `--config`
- `--bind-host`
- `--bind-port`
- `--issuer-base-url`
- `--log-level`
- `--health-endpoint-enabled`
- `--runtime-client-registration-enabled`

Precedence order is:

1. built-in defaults
2. YAML file
3. environment variables
4. CLI flags

## Recommended starting point

Use `configs/mock-server.yaml` as the main project config and keep environment-specific variations beside it.

## Top-level sections

```yaml
server:
issuer:
oauth:
token_response:
clients:
users:
claims_templates:
admin:
```

## Section reference

### `server`

Controls local process behavior.

```yaml
server:
  bind_host: 127.0.0.1
  bind_port: 8090
  log_level: info
  cors_allowed_origins: []
  startup_mode: foreground
  health_endpoint_enabled: true
  runtime_client_registration_enabled: true
  deterministic_seed: null
```

Fields:

- `bind_host`: local bind host. Default `127.0.0.1`
- `bind_port`: local bind port. Default `8090`
- `log_level`: tracing level used when `RUST_LOG` is not set. Default `info`
- `cors_allowed_origins`: allowed origins. Empty means permissive local default
- `startup_mode`: currently supports `foreground`
- `health_endpoint_enabled`: reserved for wrapper health endpoint control
- `runtime_client_registration_enabled`: enables or disables `/register`
- `deterministic_seed`: reserved for future deterministic token/key generation. Currently accepted and logged, but not yet enforced.

### `issuer`

Controls the externally visible issuer URL.

```yaml
issuer:
  base_url: http://127.0.0.1:8090
```

Fields:

- `base_url`: full issuer base URL used for discovery/JWKS and validation

Validation:

- must be a valid URL
- must include a host
- must not include a path component yet

### `oauth`

Controls OAuth/OIDC behavior exposed through the upstream engine.

```yaml
oauth:
  require_state: true
  pkce_required: false
  access_token_ttl_seconds: 3600
  refresh_token_ttl_seconds: 2592000
  authorization_code_ttl_seconds: 600
  cleanup_interval_seconds: 300
  supported_grant_types:
    - authorization_code
    - refresh_token
    - client_credentials
  supported_response_types:
    - code
  supported_scopes:
    - openid
    - profile
    - email
    - offline_access
  supported_claims:
    - sub
    - name
    - email
  token_endpoint_auth_methods:
    - client_secret_basic
    - client_secret_post
    - none
    - private_key_jwt
  code_challenge_methods:
    - plain
    - S256
  signing_algorithm: RS256
  signing_key_strategy: ephemeral_rsa
```

Fields:

- `require_state`: whether authorization requests require `state`
- `pkce_required`: reserved for later enforcement logic
- `access_token_ttl_seconds`: access token lifetime
- `refresh_token_ttl_seconds`: refresh token lifetime
- `authorization_code_ttl_seconds`: code lifetime
- `cleanup_interval_seconds`: background cleanup interval
- `supported_grant_types`: allowed grant types
- `supported_response_types`: allowed response types
- `supported_scopes`: globally supported scopes
- `supported_claims`: claims advertised by discovery
- `token_endpoint_auth_methods`: advertised token endpoint auth methods
- `code_challenge_methods`: advertised PKCE challenge methods
- `signing_algorithm`: currently must be `RS256`
- `signing_key_strategy`: currently must be `ephemeral_rsa`

Validation:

- TTL values must be greater than zero
- `signing_algorithm` currently only supports `RS256`
- `signing_key_strategy` currently only supports `ephemeral_rsa`
- `token_endpoint_auth_methods` and `code_challenge_methods` must use supported values

### `token_response`

Controls how tokens are returned to clients.

```yaml
token_response:
  emit_json_body: true
  emit_headers:
    - header_name: Authorization
      token_field: access_token
      value_format: bearer
```

Fields:

- `emit_json_body`: keep the standard OAuth JSON response body
- `emit_headers`: optional extra response headers to emit

Header item fields:

- `header_name`: HTTP header name
- `token_field`: `access_token`, `refresh_token`, or `id_token`
- `value_format`: `bearer` or `raw`

Validation:

- header names must be valid HTTP header names
- at least one output must remain enabled, so `emit_json_body: false` requires at least one header entry

Runtime behavior:

- the global `token_response` section applies to all clients unless a client defines `token_response_override`
- `token_response_override` replaces the global token response behavior for that client
- headers may emit `access_token`, `refresh_token`, or `id_token`
- missing token fields are skipped automatically, so an `id_token` header only appears on flows that actually return an ID token
- header-only mode is supported, but JSON-body mode stays enabled by default to preserve standards-based behavior

### `clients`

Defines the applications that use the mock server.

```yaml
clients:
  - client_id: springboot-resource-server
    client_name: Spring Boot Resource Server
    enabled: true
    client_secret: null
    token_endpoint_auth_method: none
    redirect_uris: []
    grant_types:
      - client_credentials
    response_types:
      - code
    allowed_scopes:
      - openid
      - profile
      - email
    default_scopes:
      - openid
    linked_users:
      - demo-user
    token_response_override: null
    claims_template_refs:
      - spring-default
    custom_claims: {}
```

Fields:

- `client_id`: unique application identifier
- `client_name`: display name for the app
- `enabled`: whether the client is active
- `client_secret`: optional secret for confidential clients
- `token_endpoint_auth_method`: for example `none` or `client_secret_basic`
- `redirect_uris`: redirect URIs for browser flows
- `grant_types`: grant types the client may use
- `response_types`: response types the client expects
- `allowed_scopes`: scopes the client may request
- `default_scopes`: default scopes to assume
- `linked_users`: users this client can operate with in test scenarios
- `token_response_override`: per-client token response behavior override
- `claims_template_refs`: named reusable claim bundles
- `custom_claims`: claim overrides specific to this client

Validation:

- `client_id` must be unique
- `linked_users` must refer to declared users
- grants, response types, and scopes must all exist in the `oauth` section
- `default_scopes` must be a subset of `allowed_scopes` when `allowed_scopes` is provided
- protected claims cannot be overridden through `custom_claims`

Startup behavior:

- enabled clients are **preloaded into the in-memory OAuth store on startup**
- disabled clients are ignored
- `/register` is optional and only needed for dynamic/manual registration scenarios
- for most local integration testing, preloading clients in YAML is the preferred path
- `token_response_override`, when present, replaces the global `token_response` behavior for that client only

### `users`

Defines test identities.

```yaml
users:
  - user_id: demo-user
    sub: demo-user
    username: demo-user
    email: demo@example.com
    display_name: Demo User
    enabled: true
    default_scopes:
      - openid
      - profile
    roles:
      - USER
    groups: []
    claims_template_refs:
      - spring-default
    custom_claims: {}
```

Fields:

- `user_id`: unique local identifier
- `sub`: token subject
- `username`: login/display username
- `email`: email claim value
- `display_name`: user-friendly display name
- `enabled`: whether the test user is active
- `default_scopes`: user-scoped defaults
- `roles`: role names
- `groups`: group names
- `claims_template_refs`: reusable claim bundles applied to the user
- `custom_claims`: user-specific custom claims

Validation:

- `user_id` must be unique
- `sub` must not be empty
- `default_scopes` must be declared in the `oauth` section
- protected claims cannot be overridden through `custom_claims`

Startup behavior:

- enabled users are loaded into the wrapper's runtime selection logic
- the **first enabled user** becomes the default authorization-flow identity
- if no enabled user exists, the upstream fallback user is still used
- `linked_users` on clients are validated now and reserved for richer per-client user selection later

### `claims_templates`

Reusable named claim bundles.

```yaml
claims_templates:
  spring-default:
    name: Demo User
    authorizations: []
```

Use this to define common claims once, then reference them from clients or users through `claims_template_refs`.

Validation:

- protected claims cannot be overridden in templates

Runtime behavior:

- claims are merged in this order: client templates, client custom claims, standard user claims, user templates, user custom claims
- later sources win when the same key appears multiple times
- access-token custom claims are currently applied on wrapper-owned `/token` flows for `authorization_code`, `refresh_token`, and `client_credentials`
- `device/token` still uses the embedded upstream behavior today and does not yet inject custom claims

### `admin`

Controls helper/admin surfaces planned for later tasks.

```yaml
admin:
  reset_endpoint_enabled: false
  list_clients_endpoint_enabled: false
  config_endpoint_enabled: false
```

Current note:

- these fields now control the helper/admin routes
- admin routes are only mounted for loopback bind hosts such as `127.0.0.1`, `::1`, or `localhost`

Current admin endpoints:

- `GET /admin/clients` when `admin.list_clients_endpoint_enabled` is `true`
- `POST /admin/reset` when `admin.reset_endpoint_enabled` is `true`
- `GET /admin/config` when `admin.config_endpoint_enabled` is `true`

## Protected claims

These claims are reserved and cannot be overridden through `custom_claims` or `claims_templates`:

- `iss`
- `aud`
- `exp`
- `iat`
- `jti`

Example:

```yaml
claims_templates:
  spring-default:
    authorizations: []

clients:
  - client_id: api-client
    client_name: API Client
    claims_template_refs:
      - spring-default
    custom_claims:
      tenant: dev

users:
  - user_id: demo-user
    sub: demo-user
    username: demo
    display_name: Demo User
    claims_template_refs:
      - spring-default
    custom_claims:
      permissions:
        orders:
          - read
          - write
```

## Minimal config example

```yaml
server:
  bind_host: 127.0.0.1
  bind_port: 8090

issuer:
  base_url: http://127.0.0.1:8090
```

## Spring Boot-oriented example

See `configs/mock-server.springboot.yaml`.
