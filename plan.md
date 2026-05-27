# Standalone OAuth Mock Server Plan

## Objective

Build a lightweight standalone Rust OAuth/OIDC mock server for local Spring Boot integration testing by wrapping [`oauth2-test-server`](https://github.com/rust-mcp-stack/oauth2-test-server), while adding:

1. a configurable application/client registration model
2. configurable token response header mapping
3. configurable JWT claim customization, including arbitrary JSON claims such as `authorizations: []`

## Overall delivery strategy

Use `oauth2-test-server` as the OAuth/OIDC protocol engine and build a thin wrapper around it for:

- configuration loading and validation
- client app registration and seeding
- user and claims modeling
- custom token response shaping
- Spring Boot-focused local testing ergonomics

This keeps the implementation small and avoids rebuilding OAuth flows from scratch.

## Execution rule

Implementation will proceed **one task at a time**.

1. We will implement **Task 1** first.
2. After Task 1 is complete, we will stop and present the result for your review.
3. We will **not** start Task 2 or any later task until you explicitly approve moving forward.
4. The same review gate applies after every subsequent task.

## Task breakdown

### Task 1 - Dependency and wrapper spike

**Goal:** prove that `oauth2-test-server` can be embedded and extended cleanly.

**Work items**

1. Add `oauth2-test-server` as a dependency.
2. Verify the crate can be started from our binary.
3. Identify how to access or wrap:
   - issuer configuration
   - router construction
   - client registration
   - token issuance
   - JWT claim construction
4. Decide whether the project will:
   - wrap upstream as-is
   - wrap with a small compatibility adapter
   - fork upstream only if extension points are insufficient

**Done when**

- we know the exact integration strategy
- we know whether custom headers and custom JWT claims can be implemented in the wrapper or require upstream changes

**Review gate**

- stop after Task 1 implementation and wait for your approval before Task 2

**Task 1 findings from the spike**

1. `oauth2-test-server` can be embedded directly as a crate and started in-process through public APIs such as `AppState::new(...).start()`.
2. The upstream crate exposes public modules for router/state/handlers, so a wrapper can own the outer binary and still reuse upstream request handling.
3. Custom token response headers look feasible **without forking** because the token handler is publicly reachable and the wrapper can post-process or replace the `/token` route.
4. Custom JWT claims do **not** currently have an obvious first-class extension hook in upstream token issuance because access-token claim construction is hard-coded in `issue_jwt(...)`.
5. Based on the current public API, custom JWT claims will likely require one of these approaches:
   - a wrapper-owned replacement token issuance path
   - a small upstream patch
   - or a fork if we want to avoid duplicating token logic

**Recommended decision after Task 1**

1. Do **not** use a build-time source patch step. It is brittle, harder to debug, and makes local builds less predictable.
2. Prefer a **small maintained fork or patched git dependency** first, with the goal of keeping the diff narrow and upstreamable.
3. Keep the wrapper project separate from the upstream patch:
   - wrapper logic stays in this repository
   - upstream customization stays in a small fork or `patch.crates-io` override
4. Only replace the full token issuance path in the wrapper if the upstream patch turns out to be larger than expected.

**Repository layout decision**

- Rust source will live under `crate/`
- the example Spring Boot application will live in this repository under `examples/`
- the Spring Boot example will validate tokens issued by the Rust mock through the mock issuer and JWKS endpoints

---

### Task 2 - Project architecture and module layout

**Goal:** define the crate structure before implementation grows.

**Work items**

1. Create the top-level module plan for:
   - bootstrap / main
   - config
   - upstream adapter
   - client registry
   - user registry
   - claims engine
   - token response mapper
   - admin/helper endpoints
2. Define the repository layout:
   - `crate/` for the Rust workspace member and source code
   - `examples/springboot-v4-resource-server/` for the example Spring Boot app
   - optional shared top-level docs/config fixtures as needed
3. Decide whether the wrapper owns the Axum router and mounts upstream routes, or proxies to an upstream router.
4. Define shared application state and synchronization strategy for in-memory data.

**Proposed repository structure for review before implementation**

```text
.
├── plan.md
├── crate/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── app/
│   │   │   ├── mod.rs
│   │   │   ├── state.rs
│   │   │   └── startup.rs
│   │   ├── config/
│   │   │   ├── mod.rs
│   │   │   ├── loader.rs
│   │   │   ├── model.rs
│   │   │   └── validate.rs
│   │   ├── upstream/
│   │   │   ├── mod.rs
│   │   │   ├── adapter.rs
│   │   │   └── patching.rs
│   │   ├── registry/
│   │   │   ├── mod.rs
│   │   │   ├── clients.rs
│   │   │   └── users.rs
│   │   ├── claims/
│   │   │   ├── mod.rs
│   │   │   ├── merge.rs
│   │   │   └── protect.rs
│   │   ├── http/
│   │   │   ├── mod.rs
│   │   │   ├── router.rs
│   │   │   ├── token_response.rs
│   │   │   ├── admin.rs
│   │   │   └── health.rs
│   │   └── error.rs
│   └── tests/
│       ├── smoke.rs
│       └── integration/
├── examples/
│   └── springboot-v4-resource-server/
│       ├── build.gradle.kts
│       ├── settings.gradle.kts
│       └── src/
│           ├── main/
│           └── test/
└── configs/
    ├── mock-server.minimal.yaml
    └── mock-server.springboot.yaml
```

**Proposed architecture decisions**

1. The wrapper should own the outer Axum router.
2. Upstream OAuth routes should be mounted through an adapter layer so we can override `/token` behavior later without rewriting unrelated endpoints.
3. Shared application state should live in `app::state` and hold:
   - loaded wrapper config
   - upstream `AppState` or adapter handle
   - client and user registries
   - claim merge policy
   - token response policy
4. The Spring Boot example should start as a resource server that validates JWTs through issuer discovery and JWKS, because that is the smallest useful integration slice.

**Done when**

- there is one agreed source layout and request flow

**Review gate**

- stop after Task 2 implementation and wait for your approval before Task 3

**Task 2 implementation result**

1. The repository now uses a workspace layout with Rust code under `crate/`.
2. The wrapper structure is in place across `app/`, `config/`, `upstream/`, `registry/`, `claims/`, and `http/`.
3. The wrapper now owns the outer router and adds a local `/health` endpoint before merging the upstream OAuth/OIDC router.
4. A Spring Boot example skeleton now exists under `examples/springboot-v4-resource-server/`.
5. Unit and smoke tests were added for config defaults/validation, router health exposure, patch strategy, claim policy placeholders, and server startup behavior.
6. A root `README.md` was added with usage instructions and a quick design overview.

---

### Task 3 - Root configuration schema

**Goal:** define the config contract the mock-server user will provide.

**Work items**

1. Create a typed root config with sections for:
   - server
   - issuer
   - oauth
   - token_response
   - clients
   - users
   - claims_templates
   - admin
2. Use **YAML** as the primary and only user-facing config format.
3. Support explicit defaults so users can provide a minimal config.
4. Add startup validation and useful error messages for invalid config.

**Done when**

- users can understand what to configure without reading source code

**Review gate**

- stop after Task 3 implementation and wait for your approval before Task 4

**Task 3 analysis**

Based on the current project goals, the config needs to cover four distinct concerns:

1. **Server runtime** - where the mock binds, how it logs, and which helper endpoints are enabled.
2. **Issuer and OAuth behavior** - issuer URL, grants, scopes, TTLs, and protocol-level defaults.
3. **Mocked identities and clients** - the client apps that use the mock, the test users they can act as, and per-client behavior overrides.
4. **Non-standard test compatibility** - custom JWT claims and token response header mapping for client apps that do not consume the default OAuth body shape.

That leads to the following proposed YAML contract.

**Proposed top-level YAML sections**

- `server`
- `issuer`
- `oauth`
- `token_response`
- `clients`
- `users`
- `claims_templates`
- `admin`

**Proposed minimal YAML**

```yaml
server:
  bind_host: 127.0.0.1
  bind_port: 8090

issuer:
  base_url: http://127.0.0.1:8090

clients: []
users: []
claims_templates: {}
```

**Proposed full YAML shape**

```yaml
server:
  bind_host: 127.0.0.1
  bind_port: 8090
  log_level: info
  cors_allowed_origins: []
  health_endpoint_enabled: true
  runtime_client_registration_enabled: true

issuer:
  base_url: http://127.0.0.1:8090
  issuer_path_prefix: ""

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

token_response:
  emit_json_body: true
  emit_headers: []
  # Example:
  # emit_headers:
  #   - header_name: Authorization
  #     token_field: access_token
  #     value_format: bearer

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
    claims_template_refs: []
    custom_claims: {}

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
    roles: []
    groups: []
    custom_claims: {}

claims_templates:
  default-user:
    name: Demo User
    authorizations: []

admin:
  reset_endpoint_enabled: false
  list_clients_endpoint_enabled: false
  config_endpoint_enabled: false
```

**Proposed defaults**

- `server.bind_host`: `127.0.0.1`
- `server.bind_port`: `8090`
- `server.log_level`: `info`
- `server.cors_allowed_origins`: `[]` meaning allow any origin for local development
- `server.health_endpoint_enabled`: `true`
- `server.runtime_client_registration_enabled`: `true`
- `issuer.base_url`: derived from bind host/port unless explicitly set
- `oauth.require_state`: `true`
- `oauth.pkce_required`: `false`
- `oauth.access_token_ttl_seconds`: `3600`
- `oauth.refresh_token_ttl_seconds`: `2592000`
- `oauth.authorization_code_ttl_seconds`: `600`
- `oauth.cleanup_interval_seconds`: `300`
- `token_response.emit_json_body`: `true`
- `token_response.emit_headers`: `[]`
- `clients`: `[]`
- `users`: `[]`
- `claims_templates`: `{}`
- `admin.*`: `false` by default

**Validation rules to implement**

1. `issuer.base_url` must be a valid URL.
2. Client IDs must be unique.
3. User IDs must be unique.
4. `linked_users` must refer to declared users.
5. Header names in `token_response.emit_headers` must be valid HTTP header names.
6. Protected claims such as `iss`, `aud`, `exp`, `iat`, and `jti` cannot be overridden through `custom_claims`.
7. TTL values must be positive integers.
8. `grant_types`, `response_types`, and scopes must stay within supported values declared under `oauth`.

**Recommendation before implementation**

Start Task 3 implementation with:

1. `server`
2. `issuer`
3. `oauth`
4. `token_response`

Then add `clients`, `users`, and `claims_templates` once the top-level schema is approved.

**Task 3 implementation result**

1. The YAML schema is now implemented in Rust with typed sections for `server`, `issuer`, `oauth`, `token_response`, `clients`, `users`, `claims_templates`, and `admin`.
2. The application now loads YAML from `O2MS_CONFIG`, then `configs/mock-server.yaml`, then built-in defaults.
3. Validation now covers issuer URL correctness, TTL values, duplicate client/user IDs, unknown linked users, invalid header names, unsupported grant/scope/response values, and protected-claim overrides.
4. Example configs were added under `configs/` for minimal, default, and Spring Boot-oriented setups.
5. Extensive documentation was added under `docs/` and linked from `README.md`.

---

### Task 4 - Server and runtime settings

**Goal:** make the standalone server easy to run locally.

**Planned settings**

- bind host
- bind port
- externally advertised issuer URL
- log level
- allowed CORS origins
- config file path
- startup mode
- runtime registration enabled/disabled
- reset endpoint enabled/disabled
- health endpoint enabled/disabled
- deterministic seed enabled/disabled

**Work items**

1. Define defaults for local machine usage.
2. Decide which values can be overridden via environment variables.
3. Print key startup URLs clearly for local developers.

**Done when**

- the server can be started locally with a small config and predictable defaults

**Review gate**

- stop after Task 4 implementation and wait for your approval before Task 5

**Task 4 implementation result**

1. Server/runtime settings now include `startup_mode`, `health_endpoint_enabled`, `runtime_client_registration_enabled`, and `deterministic_seed`.
2. YAML settings can now be selectively overridden through environment variables for bind host/port, log level, CORS, issuer URL, route gating, and token TTLs.
3. The wrapper router now actually enforces runtime flags, including optional `/health` exposure and optional `/register` exposure.
4. Startup logs now show the active runtime settings and explicitly log when routes are disabled.
5. Documentation was updated to explain config source precedence and runtime overrides.

---

### Task 5 - OAuth and issuer behavior settings

**Goal:** expose the important parts of issuer behavior without forcing code changes.

**Planned settings**

- issuer URL
- supported grant types
- supported response types
- supported scopes
- supported claims
- token endpoint auth methods
- PKCE requirements
- require `state`
- access token TTL
- refresh token TTL
- authorization code TTL
- cleanup interval
- signing algorithm
- signing key strategy

**Work items**

1. Map wrapper config to the upstream `IssuerConfig`.
2. Identify which upstream defaults should be preserved.
3. Decide which protocol fields remain fixed versus user-configurable.

**Done when**

- issuer behavior can be tuned through config rather than source edits

**Review gate**

- stop after Task 5 implementation and wait for your approval before Task 6

**Task 5 implementation result**

1. OAuth/issuer settings now map more completely into the upstream `IssuerConfig`, including token endpoint auth methods, PKCE challenge methods, scope/claim advertisement, TTLs, and required state behavior.
2. Validation now enforces the currently supported signing contract: `RS256` and `ephemeral_rsa`.
3. The issuer URL is now validated as a root base URL, which prevents unsupported path-prefix configurations from silently behaving incorrectly.
4. New tests cover runtime/OAuth mapping, disabled route behavior, env override application, and unsupported OAuth setting rejection.

---

### Task 6 - Client application registration model

**Goal:** allow mock-server users to register their applications through config JSON and optional runtime APIs.

**Each client app should be able to define**

- client name
- client id
- client secret or secret generation behavior
- redirect URIs
- grant types
- response types
- scopes
- token endpoint auth method
- enabled/disabled state
- linked test users
- token response header overrides
- claim overrides

**Work items**

1. Define the JSON shape for a client app definition.
2. Support startup-seeded clients from config.
3. Support optional runtime registration from JSON payloads.
4. Define inheritance rules between server defaults and per-client overrides.

**Done when**

- a user can register at least one Spring Boot client app without touching Rust code

**Review gate**

- stop after Task 6 implementation and wait for your approval before Task 7

**Task 6 implementation result**

1. YAML `clients:` entries are now seeded into the upstream in-memory OAuth store at startup.
2. Disabled clients are skipped during seeding.
3. Preloaded clients can use OAuth flows immediately without calling `/register`.
4. `/register` remains optional and controlled by `server.runtime_client_registration_enabled`.
5. Validation now ensures `default_scopes` stay within `allowed_scopes` when client restrictions are declared.

---

### Task 7 - Test user model

**Goal:** allow repeatable authentication scenarios with explicit test identities.

**Each user should be able to define**

- `sub`
- username
- email
- display name
- roles
- groups
- default scopes
- enabled/disabled state
- arbitrary extra claims

**Work items**

1. Define the user config schema.
2. Allow clients to reference one or more test users.
3. Decide whether one default fallback user exists when no user is configured.

**Done when**

- the mock can issue tokens for named test users consistently

**Review gate**

- stop after Task 7 implementation and wait for your approval before Task 8

**Task 7 implementation result**

1. Enabled YAML `users:` now influence runtime behavior instead of being schema-only.
2. The first enabled configured user becomes the upstream default authorization-flow identity.
3. If no enabled user is configured, the server falls back to the upstream built-in default user.
4. Documentation now explains how `users:` affect authorization and userinfo behavior today.
5. Integration coverage now proves that a configured user becomes the subject used by authorization-code flow tokens.

---

### Task 8 - Custom JWT claims engine

**Goal:** let the mock-server user customize JWT payloads, including arrays and nested JSON structures.

**Required capability**

The user must be able to add claims such as:

```json
{
  "authorizations": [],
  "roles": ["ADMIN", "USER"],
  "tenant": "dev",
  "permissions": {
    "orders": ["read", "write"]
  }
}
```

**Work items**

1. Define how claims can be supplied:
   - global defaults
   - per-user claims
   - per-client claims
   - request-scoped overrides if we decide to support them
2. Define merge precedence, for example:
   - server defaults
   - client defaults
   - user claims
   - request overrides
3. Implement the upstream customization strategy for claims support:
   - add the small maintained fork or `patch.crates-io` override here
   - keep the upstream diff narrowly focused on claim injection hooks
   - avoid build-time patch scripts
   - only fall back to wrapper-owned token issuance if the upstream patch becomes too invasive
4. Protect protocol-owned claims such as:
   - `iss`
   - `aud`
   - `exp`
   - `iat`
   - `jti`
5. Decide whether custom claims apply to:
   - access token only
   - ID token only
   - both
6. Decide how claims are rendered when values are arrays, objects, booleans, or null.

**Done when**

- arbitrary JSON claims can be configured safely and show up in issued JWTs as expected
- the fork or patched dependency is wired into the build and limited to the minimum hook surface needed for custom claims

**Review gate**

- stop after Task 8 implementation and wait for your approval before Task 9

**Task 8 implementation result**

1. The wrapper now owns `/token` for `authorization_code`, `refresh_token`, and `client_credentials` grants so it can inject custom JWT claims safely.
2. Claim values now support arbitrary JSON, including arrays, objects, booleans, strings, and nulls.
3. Claim precedence is now implemented as: client templates, client custom claims, standard user claims, user templates, then user custom claims.
4. Protected protocol claims remain blocked from configuration overrides.
5. Integration coverage now proves configured custom claims appear in issued JWT access tokens.

---

### Task 9 - Token response customization

**Goal:** support non-standard client expectations around where the bearer token is returned.

**Required capability**

The mock should be able to return the token:

- in the standard JSON body
- in a configurable response header
- or in both places

**Planned options**

- header name, for example `Authorization` or `X-Auth-Token`
- header format, for example `Bearer <token>` or raw token
- access token only versus multiple tokens
- global default behavior
- per-client override behavior

**Work items**

1. Define the `token_response` config schema.
2. Decide whether standard JSON fields remain enabled by default when headers are added.
3. Decide whether refresh token and ID token may also be emitted as headers.
4. Implement response shaping without breaking standards-based defaults.

**Done when**

- a client can be configured to read its token from the body, a header, or both

**Review gate**

- stop after Task 9 implementation and wait for your approval before Task 10

**Task 9 implementation result**

1. The wrapper-owned `/token` endpoint now supports shaping responses with the standard JSON body, configured headers, or both.
2. Header emission now supports `access_token`, `refresh_token`, and `id_token` fields in either `Bearer <token>` or raw-token format.
3. Global `token_response` behavior is now applied by default, and `clients[].token_response_override` fully replaces it for specific clients.
4. Validation now rejects token response configurations that disable the JSON body without defining at least one header.
5. Unit and smoke coverage now prove header-only responses and per-client override behavior.

---

### Task 10 - Runtime registration and helper APIs

**Goal:** make the mock convenient for iterative local testing.

**Candidate endpoints**

- `POST /register` for runtime client registration
- `GET /admin/clients` to inspect registered apps
- `POST /admin/reset` to clear in-memory state
- `GET /health` for startup checks
- optional test-only endpoint for direct token minting

**Work items**

1. Decide which helper endpoints are enabled by default.
2. Define safe local-only behavior for admin endpoints.
3. Decide whether reset also clears seeded clients or only runtime state.

**Done when**

- the mock can support both stable local configs and rapid manual testing

**Review gate**

- stop after Task 10 implementation and wait for your approval before Task 11

**Task 10 implementation result**

1. The wrapper now exposes optional admin/helper routes for listing clients, resetting runtime state, and inspecting the loaded config.
2. Admin routes are disabled by default and only mount when explicitly enabled on a loopback bind host.
3. Reset now clears transient runtime state and reseeds configured YAML clients so stable local setups survive manual experimentation.
4. Automated coverage now proves runtime registration appears in admin listings and reset removes runtime clients while keeping configured ones.

---

### Task 11 - Spring Boot compatibility layer

**Goal:** make the first supported integration path smooth for Spring Boot applications.

**Work items**

1. Identify the first Spring Boot app types to support, for example:
   - OAuth2 client
   - resource server
   - machine-to-machine client
   - web app using authorization code flow
2. Prepare example Spring Boot configuration for:
   - issuer URI
   - client registration
   - redirect URI
   - JWKS validation
3. Add an example Spring Boot v4 application under `examples/` that validates JWTs from this mock server.
4. Use **Maven** for the Spring Boot example project structure and keep it on the latest available Spring Boot 4 release at implementation time.
5. Decide how custom token response headers are expected to be consumed by those clients.

**Done when**

- there is at least one documented happy path from Spring Boot to the mock server

**Review gate**

- stop after Task 11 implementation and wait for your approval before Task 12

**Task 11 implementation result**

1. The example application under `examples/springboot-v4-resource-server/` now uses Maven with Spring Boot 4.0.6.
2. The example is now an explicit JWT resource server with a small security configuration and a controller that surfaces custom claims.
3. Spring documentation now explains how to run the mock server, run the Maven example, request a token, and call the example API.

---

### Task 12 - Automated testing

**Goal:** keep the mock reliable as configurability grows.

**Test coverage**

1. config parsing
2. default values
3. invalid config errors
4. seeded client registration
5. runtime client registration
6. authorization code flow
7. client credentials flow
8. discovery endpoint
9. JWKS endpoint
10. custom JWT claims
11. array/object claims such as `authorizations: []`
12. custom token response headers
13. reset behavior
14. Spring Boot-facing happy-path scenarios where practical

**Done when**

- the most important configuration and integration surfaces are covered

**Review gate**

- stop after Task 12 implementation and wait for your approval before Task 13

**Task 12 implementation result**

1. Test coverage now includes helper/admin route mounting, runtime client registration visibility, reset behavior, CLI parsing, and CLI-overridden config loading.
2. The Spring Boot example now has Maven-backed tests that prove the example app can read JWT claims as expected.

---

### Task 13 - Packaging and local developer UX

**Goal:** keep the server lightweight and easy to run.

**Work items**

1. Add example config files.
2. Add CLI flags for config path and runtime options.
3. Keep startup fast and the binary small.
4. Print useful startup diagnostics only.

**Done when**

- a developer can run the mock locally with one command and a simple config file

**Review gate**

- stop after Task 13 implementation and wait for your approval before any follow-up scope

**Task 13 implementation result**

1. The binary now supports CLI flags for config path and common runtime overrides on top of the existing YAML and environment-variable configuration model.
2. CLI overrides now apply after file and environment loading so one-command local runs stay easy and predictable.
3. Documentation now explains the supported flags and their precedence relative to files and environment variables.

## Immediate next step

Start with **Task 1 - Dependency and wrapper spike** only.

That task will answer the critical first questions:

1. Can `oauth2-test-server` be embedded cleanly in this project?
2. Can custom JWT claims be added without a fork?
3. Can custom token response headers be added without a fork?
4. Is a wrapper enough, or do we need a small upstream patch/fork?

## Decisions to iterate with you next

These are the questions we should work through one by one before implementation:

1. What are the different client app types that will use this mock?
2. What configuration sections do you want users to provide?
3. What default values should each configuration section have?
4. Which JWT claims should exist by default?
5. Which JWT claims should be configurable per client, per user, or globally?
6. What should the default token response header behavior be?
