# API reference

This document describes the HTTP endpoints currently exposed by the mock server.

## Overview

The server exposes two kinds of routes:

1. **Wrapper routes** implemented directly by this repository
2. **OAuth/OIDC routes** delegated to the embedded `oauth2-test-server`

The wrapper currently adds configuration-aware behavior such as:

- optional `/health`
- optional runtime registration routes
- CORS settings
- startup client seeding from YAML

## Base URL

By default, the local base URL is:

```text
http://127.0.0.1:8090
```

This can be changed through YAML or environment variables.

## Route summary

| Method | Path | Source | Optional | Purpose |
|---|---|---|---|---|
| GET | `/health` | wrapper | yes | Basic health probe |
| GET | `/.well-known/openid-configuration` | upstream | no | OIDC discovery |
| GET | `/.well-known/jwks.json` | upstream | no | JWKS document |
| POST | `/register` | upstream | yes | Dynamic client registration |
| GET | `/register/{client_id}` | upstream | yes | Retrieve registered client |
| GET | `/authorize` | upstream | no | Authorization endpoint |
| POST | `/token` | wrapper | no | Token endpoint with custom-claim injection |
| POST | `/device/code` | upstream | no | Device authorization request |
| POST | `/device/token` | upstream | no | Device token polling |
| POST | `/introspect` | upstream | no | Token introspection |
| POST | `/revoke` | upstream | no | Token revocation |
| GET | `/userinfo` | upstream | no | UserInfo endpoint |
| GET | `/error` | upstream | no | Simple error page |
| GET | `/admin/clients` | wrapper | yes | List current preloaded and runtime clients |
| POST | `/admin/reset` | wrapper | yes | Clear runtime state and reseed configured clients |
| GET | `/admin/config` | wrapper | yes | Inspect the active loaded config |

## Wrapper-owned endpoint

### `GET /health`

Returns a simple health response.

Example response:

```text
ok
```

Status:

- `200 OK` when enabled
- `404 Not Found` when `server.health_endpoint_enabled` is `false`

## OIDC discovery endpoints

### `GET /.well-known/openid-configuration`

Returns the OpenID Connect discovery document derived from the configured issuer.

Important fields include:

- `issuer`
- `authorization_endpoint`
- `token_endpoint`
- `userinfo_endpoint`
- `jwks_uri`
- `registration_endpoint`
- `revocation_endpoint`
- `introspection_endpoint`
- supported grants, scopes, claims, response types, and token auth methods

### `GET /.well-known/jwks.json`

Returns the public signing keys used to validate issued JWTs.

This is the main endpoint your Spring Boot resource server uses for JWT verification.

## Client registration endpoints

### `POST /register`

Creates a client dynamically at runtime using RFC 7591-style metadata.

This route is **optional**:

- available when `server.runtime_client_registration_enabled` is `true`
- absent when `server.runtime_client_registration_enabled` is `false`

Important note:

- most local test setups should prefer **preloaded YAML clients**
- `/register` is mainly for dynamic or manual testing scenarios

Typical request shape:

```json
{
  "redirect_uris": ["http://localhost:8080/login/oauth2/code/mock"],
  "grant_types": ["authorization_code"],
  "response_types": ["code"],
  "scope": "openid profile email"
}
```

Typical response shape:

```json
{
  "client_id": "...",
  "client_secret": "...",
  "client_id_issued_at": 0,
  "registration_client_uri": "http://127.0.0.1:8090/register/...",
  "registration_access_token": "...",
  "redirect_uris": [],
  "grant_types": [],
  "response_types": [],
  "scope": "",
  "token_endpoint_auth_method": "client_secret_basic"
}
```

### `GET /register/{client_id}`

Retrieves metadata for a registered client.

When available, this can return:

```json
{
  "client_id": "springboot-resource-server",
  "client_name": "Spring Boot Resource Server",
  "redirect_uris": [],
  "grant_types": ["client_credentials"],
  "scope": "openid profile email"
}
```

## Authorization endpoint

### `GET /authorize`

Authorization code flow entry point.

Handled by the embedded upstream server. This repository currently relies on the upstream behavior, including:

- `state` enforcement from config
- redirect URI validation
- scope validation
- PKCE challenge handling

Important current behavior:

- consent is auto-granted for test purposes
- when `oauth.authorization_user_picker_enabled` is `true`, the mock shows a simple local picker page so the tester can choose an enabled YAML user before code issuance
- when the picker is disabled, the mock uses the **first enabled configured user** as the default authorization-flow identity
- if no enabled configured user exists, it falls back to the upstream built-in default user

## Token endpoint

### `POST /token`

Issues tokens for supported grant types.

Supported grant types currently come from config and are mapped into the upstream issuer settings.

Currently supported by default:

- `authorization_code`
- `refresh_token`
- `client_credentials`

Example client credentials request:

```bash
curl -X POST http://127.0.0.1:8090/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=seeded-client&scope=openid"
```

Typical response shape:

```json
{
  "access_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "scope": "openid"
}
```

Example header emission:

```http
Authorization: Bearer <access-token>
X-Refresh-Token: <refresh-token>
```

Important note:

- if the client was preloaded from YAML, it can use `/token` immediately
- `/register` is not required for preloaded clients
- custom claims from `claims_templates`, client `custom_claims`, and user `custom_claims` are injected into access tokens on wrapper-owned grants
- current wrapper-owned grant handling covers `authorization_code`, `refresh_token`, and `client_credentials`
- token responses can now be emitted in the JSON body, configured headers, or both
- header emission is controlled by global `token_response` settings and optional per-client `token_response_override`
- supported header token sources are `access_token`, `refresh_token`, and `id_token`
- `device/token` still uses the embedded upstream implementation and does not yet inject custom claims

## Device flow endpoints

### `POST /device/code`

Starts a device authorization flow.

### `POST /device/token`

Polls for a device flow token.

These routes are available because the upstream server supports device flow.

## Admin/helper endpoints

These routes are disabled by default and are only mounted when their `admin.*` config flags are enabled **and** the server is bound to a loopback host.

### `GET /admin/clients`

Returns the current client list from the in-memory store.

Each entry includes a `source` field:

- `preloaded` for YAML-seeded clients
- `runtime` for clients added through `/register`

### `POST /admin/reset`

Clears transient runtime state and then reseeds configured clients from YAML.

Current reset behavior:

- clears authorization codes
- clears access tokens
- clears refresh tokens
- clears device flow state
- removes runtime-registered clients
- restores preloaded configured clients

### `GET /admin/config`

Returns the active loaded application config as JSON.

## Token management endpoints

### `POST /introspect`

Returns token activity and claims information.

### `POST /revoke`

Revokes an access token or refresh token.

## User info endpoint

### `GET /userinfo`

Returns user info for a valid bearer token.

Expected request header:

```http
Authorization: Bearer <access-token>
```

Current behavior:

- for authorization-code/device-style user flows, the `sub` value comes from the effective default configured user
- for client credentials flow, the token subject remains the upstream client-style subject

## Error endpoint

### `GET /error`

Returns a simple HTML error page used by upstream OAuth redirect flows.

## Preloaded clients vs runtime registration

There are two ways clients can exist in the server:

1. **Preloaded clients from YAML**
   - loaded at startup
   - preferred for stable local integration testing
   - available immediately for `/token` and other flows

2. **Runtime-registered clients via `/register`**
   - optional
   - useful for manual or dynamic test scenarios

For most Spring Boot cases, use **preloaded YAML clients**.

## Configuration flags that affect the API

### `server.health_endpoint_enabled`

- `true`: `/health` exists
- `false`: `/health` is not exposed

### `server.runtime_client_registration_enabled`

- `true`: `/register` and `/register/{client_id}` exist
- `false`: both registration routes are absent

### `server.cors_allowed_origins`

Controls CORS policy for the exposed routes.

### `issuer.base_url`

Controls the issuer shown in discovery and token validation flows.

### `oauth.*`

Controls:

- token TTLs
- `require_state`
- supported grants
- supported response types
- advertised scopes and claims
- token endpoint auth methods
- PKCE challenge methods
- signing metadata

## Current API limitations

At the moment:

- custom JWT claim injection is **not implemented yet**
- token-header response customization is **not implemented yet**
- admin endpoints are configured in schema but **not implemented yet**
- the Spring Boot example exists, but the full end-to-end integration task is still pending

## Recommended usage pattern

For local application testing:

1. define clients and users in YAML
2. start the mock server
3. point your Spring Boot app to the issuer URL
4. use the preloaded client immediately

Use `/register` only when you specifically need dynamic client creation.
