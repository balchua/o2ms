# API Gateway + OAuth Feature Design (Dev/Test Lightweight Mode)

## 1) Current system review (baseline)

This repository already provides:

- A Rust Axum wrapper server (`crate/src/http/router.rs`) with local routes.
- Embedded OAuth/OIDC behavior delegated to `oauth2-test-server` via adapter/state (`crate/src/upstream/adapter.rs`).
- YAML-first configuration (`docs/configuration.md`, `crate/src/config/model.rs`, `crate/src/config/loader.rs`) with `O2MS_` nested env overrides.
- Wrapper-owned token shaping features and configurable header emission on token responses.
- Admin helper routes and startup client seeding from YAML.

Implication for this feature:

- OAuth server behavior already exists and should remain the base.
- API-gateway behavior should be added as a thin wrapper module, not a heavy reverse-proxy platform.
- Configuration must stay YAML-centric and consistent with existing schema/validation patterns.

---

## 2) Feature objective

Add a **simple, lightweight API gateway mode** for development/testing that can run as:

1. **OAuth-only mode** (current behavior, no API proxying)
2. **Combined mode** (OAuth server + API gateway)
3. *(Optional, pending confirmation)* **Gateway-only mode** (proxy without exposing OAuth routes)

The gateway should:

- Route configured inbound paths to configured upstream APIs.
- Forward or map bearer tokens into configurable outbound headers.
- Be easy to configure from YAML.
- Avoid high resource consumption (minimal allocations, no heavyweight plugin/runtime model).
- Include a practical Java/Spring Boot client example that calls APIs through the gateway for end-to-end dev/testing.

This is meant to emulate a basic Kong-like local gateway flow, not replace full Kong capabilities.

---

## 3) Scope and non-goals

### In scope

- Path-based proxy routing configured via YAML.
- Per-route upstream target URL config.
- Per-route and global auth forwarding strategy:
  - read bearer token from incoming request (header/cookie/query optionality to be confirmed),
  - write token to configurable outbound header name,
  - configurable value format (`bearer <token>` vs raw token).
- Mode switch(es) for OAuth-only vs combined (and maybe gateway-only).
- Basic observability and error mapping suitable for local debugging.

### Out of scope (initial version)

- Full Kong plugin system.
- Complex traffic management (rate limiting, retries with budgets, circuit breaking, canary balancing).
- Full policy engine / RBAC / ACL subsystem.
- Multi-node distributed config/state synchronization.

---

## 4) Proposed architecture

## 4.1 New modules

- `crate/src/gateway/mod.rs`  
  Core gateway module exports config mapping and proxy helpers.
- `crate/src/gateway/model.rs`  
  Internal runtime route model (compiled from config).
- `crate/src/gateway/proxy.rs`  
  Request forwarding logic and outbound request construction.
- `crate/src/gateway/auth_forward.rs`  
  Token extraction + outbound header mapping.
- `crate/src/gateway/errors.rs`  
  Gateway-specific error types and HTTP mapping.

## 4.2 Router integration

- Extend existing wrapper router builder to conditionally mount gateway routes based on config mode.
- Keep OAuth routes unchanged in OAuth-only and combined modes.
- Route match order:
  1. fixed wrapper/system routes (`/health`, admin, OAuth endpoints),
  2. configured gateway routes.
- Use explicit config validation to prevent route collisions with reserved OAuth/system paths.

## 4.3 Runtime flow (combined mode)

1. Incoming request hits configured gateway route.
2. Match route and build upstream URL (base + remaining path + query handling rules).
3. Extract bearer token (default: `Authorization` request header).
4. Write token to outbound header (default configurable, e.g. `Authorization` or `X-Forwarded-Access-Token`).
5. Forward method/body/selected headers to upstream.
6. Return upstream response status/body/headers with safe filtering policy.
7. Emit tracing logs for route match, upstream target, duration, and failures.

---

## 5) YAML configuration design

Add a top-level section:

```yaml
gateway:
  enabled: false
  mode: oauth_only # oauth_only | oauth_and_gateway
  timeout_ms: 5000
  max_response_body_bytes: 1048576
  forward_headers:
    allowlist:
      - content-type
      - accept
      - x-request-id
  auth:
    token_sources:
      authorization_header: true
      cookie_name: null
      query_param: null
    outbound:
      header_name: Authorization
      value_format: bearer # bearer | raw
      overwrite_existing_header: true
  routes:
    - id: users-api
      enabled: true
      upstream_base_url: http://127.0.0.1:9001
      inbound:
        path_prefix: /proxy/users
        strip_prefix: /proxy/users
        methods: [GET, POST, PUT, DELETE]
      auth:
        required: true
        passthrough_mode: from_request # from_request | fixed_token
        fixed_token: null
        outbound_header_name: X-Upstream-Auth
        outbound_value_format: bearer
      upstream:
        path_join_mode: safe_join
        forward_query_string: true
```

### Config semantics

- `gateway.enabled`: master switch for proxy behavior.
- `gateway.mode`:
  - `oauth_only`: no gateway route mounting.
  - `oauth_and_gateway`: OAuth + gateway routes active.
- Global `gateway.auth.outbound.*` is default, route-level values can override.
- Each `routes[]` item has independent auth settings and upstream mapping.
- Token pass-through behavior is configurable without code changes.

### Validation rules

- route `id` must be unique.
- `upstream_base_url` must be valid absolute URL.
- `path_prefix` must start with `/`.
- `strip_prefix` must be compatible with `path_prefix`.
- gateway route prefixes must not overlap reserved endpoints (`/token`, `/authorize`, `/.well-known/*`, `/admin/*`, `/health`, etc.).
- when `required=true` and `passthrough_mode=from_request`, at least one token source must be enabled.
- header names must be valid HTTP header names.

---

## 6) Lightweight resource strategy

To keep it lightweight:

- Reuse existing Axum/Tower stack and shared runtime.
- Keep config and route table in-memory with simple deterministic matching (prefix-based).
- Avoid dynamic plugin loading and heavy policy engines.
- Use one reusable async HTTP client with connection pooling and bounded timeouts.
- Add configurable response size safeguards (`max_response_body_bytes`).
- Keep default behavior conservative and local-dev friendly.

---

## 7) Security behavior (dev/test oriented)

- Do not log raw bearer tokens.
- Support required-auth vs optional-auth per route.
- Reject missing token with `401` when route requires auth.
- Return `502/504` style errors for upstream failures/timeouts.
- Header forwarding via allowlist to reduce accidental leakage.
- Optional fixed-token mode only for controlled dev/test simulation.

---

## 8) Observability and UX

- Tracing spans for each proxied request:
  - route id,
  - upstream host,
  - method/path,
  - status and duration.
- Structured warning/error logs for:
  - missing token,
  - invalid config,
  - upstream unavailability,
  - header rewrite conflicts.
- Add API documentation for gateway routes and behavior examples.

## 8.1 Example app coverage (Spring Boot + Java client)

The feature also needs runnable example coverage similar to the existing Spring Boot sample:

- Extend the Spring Boot example set with a gateway-focused scenario.
- Add a Java client flow that:
  - obtains token from the mock OAuth server,
  - calls gateway endpoint,
  - demonstrates gateway pass-through to upstream API with configurable auth-header mapping.
- Keep the Java example aligned with user preference:
  - Maven build,
  - Spring Boot 4 baseline,
  - simple local run instructions.

---

---

## 9) Detailed implementation task breakdown (short-spurt tasks)

Each task is intentionally small and independently reviewable.

### Task 1 — Add gateway config schema scaffolding

- Add `GatewayConfig` and nested structs to config model with defaults.
- Keep defaults in OAuth-only mode.
- No routing behavior changes yet.

### Task 2 — Add YAML parsing coverage for gateway config

- Add unit tests proving YAML parsing/defaults for new gateway fields.
- Include route list parsing and enum parsing for mode/value format.

### Task 3 — Add environment override mapping for gateway config

- Ensure `O2MS_GATEWAY__...` nested env overrides work.
- Add loader tests for at least one global and one per-route override.

### Task 4 — Add config validation for gateway global fields

- Validate mode value, timeout bounds, max body size bounds, header names.
- Keep error messages aligned with existing validation style.

### Task 5 — Add config validation for route definitions

- Validate unique route IDs, valid URLs, path prefix rules, auth consistency.
- Add focused tests for each validation failure type.

### Task 6 — Add reserved route collision validator

- Reject gateway route prefixes that collide with OAuth/system/admin paths.
- Add tests for known reserved endpoints.

### Task 7 — Introduce gateway runtime route model

- Create compiled route model optimized for request matching.
- Add pure unit tests for match and path rewrite behavior.

### Task 8 — Implement auth token extraction helper

- Extract token from configured source(s) with deterministic precedence.
- Return explicit typed errors for missing/malformed tokens.

### Task 9 — Implement outbound auth header formatter

- Convert token to outbound header per configured name and value format.
- Add tests for `bearer` and `raw` formats + invalid header guardrails.

### Task 10 — Implement proxy request builder

- Build upstream URI from base URL + rewritten path + optional query forwarding.
- Copy method/body and allowlisted headers.
- Unit-test URI join edge cases.

### Task 11 — Implement reusable upstream HTTP client wiring

- Add a shared async client with connect/read timeout config support.
- Keep pooling defaults lightweight.

### Task 12 — Implement proxy response translation

- Map upstream status/body/headers back to caller with filtering rules.
- Add max response body size handling with deterministic failure response.

### Task 13 — Add gateway handler and error mapping

- Implement Axum handler using route model + proxy/auth helpers.
- Map internal errors to expected HTTP status codes (`400/401/502/504`).

### Task 14 — Mount gateway routes based on mode

- Integrate into router builder:
  - OAuth-only unchanged,
  - combined mode mounts both,
  - gateway-only behavior if approved.
- Add router tests for each mode.

### Task 15 — Add integration tests for happy-path proxying

- Use local test upstream server to verify forwarding and response passthrough.
- Assert configurable outbound auth header mapping.

### Task 16 — Add integration tests for auth failures

- Missing token, malformed bearer, route-auth-required behavior.
- Verify correct status and no secret leakage in response.

### Task 17 — Add integration tests for upstream failures

- Timeout/unreachable/upstream 5xx handling and status mapping.
- Validate deterministic error body format.

### Task 18 — Update `configs/mock-server.yaml` with commented gateway examples

- Add non-breaking sample section showing defaults and one route.
- Keep gateway disabled by default.

### Task 19 — Update `docs/configuration.md` for full gateway schema

- Add `gateway` section with field reference, validation rules, and env examples.

### Task 20 — Update `docs/api.md` for gateway behavior

- Document route pattern, auth forwarding semantics, and error behaviors.

### Task 21 — Add dedicated gateway guide in `docs/`

- Add developer-focused setup and local testing flow for Kong-like emulation.
- Include OAuth-only vs combined mode examples.

### Task 22 — Add performance sanity checks

- Add lightweight benchmark or smoke checks for route matching + proxy path.
- Confirm no significant memory/cpu regressions in local dev profile.

### Task 23 — Final validation and hardening pass

- Run lint/build/test suite.
- Verify docs consistency and examples.
- Re-check token redaction in logs and security posture.

### Task 24 — Add Spring Boot gateway integration example design

- Define example topology: OAuth mock server + gateway routes + sample upstream API.
- Reuse existing Spring Boot example structure where possible to avoid heavy new project setup.
- Document expected local ports and request flow diagram.

### Task 25 — Add Java client-through-gateway implementation task

- Add Maven-based Java client sample (or Spring Boot module) that requests token and calls gateway path.
- Include configurable gateway base URL and client credentials via environment variables.
- Keep client code intentionally minimal for dev/test reproducibility.

### Task 26 — Add gateway example YAML profiles

- Add dedicated YAML config profile(s) for gateway example scenarios (OAuth-only and combined mode).
- Preconfigure at least one upstream route and auth-header forwarding behavior.
- Ensure examples stay safe-by-default and local-only.

### Task 27 — Add end-to-end example documentation set

- Update `docs/springboot.md` with a gateway-specific section and runbook.
- Add or update gateway-focused docs (for example `docs/gateway.md`) to include:
  - architecture overview,
  - config reference links,
  - Java client walkthrough,
  - troubleshooting matrix.
- Add curl and Java client verification steps for token and proxied API calls.

### Task 28 — Add example validation tasks

- Add automated smoke tests or scripted verification for the example happy path.
- Validate that example instructions are executable from a clean checkout.
- Confirm docs are synchronized with committed config/example code.

---

## 10) Rollout suggestion

- Deliver in phases:
  1. config + validation,
  2. runtime proxy/auth internals,
  3. router integration + tests,
  4. Java/Spring Boot gateway example and YAML profiles,
  5. documentation and examples.

This sequence keeps each PR small and enables short implementation spurts.

---

## 11) Clarification questions

Please confirm these before implementation:

1. Confirmed: v1 supports only `oauth_only` and `oauth_and_gateway`; OAuth remains enabled whenever gateway is enabled.
2. Should token extraction support only `Authorization: Bearer` initially, or also cookie/query sources in v1?
3. For outbound auth header, should default be `Authorization` or a custom header like `X-Forwarded-Access-Token`?
4. Should we allow **fixed token** per route for service-to-service mock scenarios, or strictly pass-through only?
5. Do you need host-based routing (by `Host` header), or is path-prefix routing enough for v1?
6. Should CORS for gateway routes reuse existing server-level CORS config, or have gateway-specific CORS controls?
7. For upstream response header forwarding, do you prefer allowlist-only (safer default) or pass-through with denylist?
8. Do you want request/response body size limits configurable per route, or global-only in v1?
9. Should gateway routes support request path rewrite templates beyond simple prefix-strip (for example `/a/* -> /v1/*`)?
10. What is your preferred default timeout for upstream calls in dev/test?
11. Should the Java example be a new Spring Boot module in `examples/` or an extension of the existing `springboot-v4-resource-server` app?
12. Do you want the Java client example to use `RestClient` only, or include `WebClient` variant too?
