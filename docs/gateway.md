# API Gateway feature

The mock server includes a lightweight API gateway mode for development and testing. It acts as a reverse proxy that validates bearer tokens against the server's own OAuth/JWT pipeline before forwarding requests to upstream APIs.

## When to use the gateway

- You want to test a client that calls an upstream API through a gateway (similar to Kong, Apigee, etc.)
- You need token validation at the gateway level before requests reach the upstream
- You want to emulate a full OAuth + Gateway topology locally without running separate services

## How it works

```
Client  ──►  Mock Server (gateway)  ──►  Upstream API
                 │
                 └── validates bearer token via local OAuth/JWT
```

1. Client obtains a token from the mock server's `/token` endpoint
2. Client calls the gateway route with `Authorization: Bearer <token>`
3. Gateway validates the token locally
4. If valid, gateway forwards the request to the configured upstream, passing the token in the configured outbound header
5. If invalid/missing, gateway returns `401` immediately

## Configuration

Add a `gateway` section to your YAML config:

```yaml
gateway:
  enabled: true
  timeout_ms: 2000
  max_body_bytes: 1048576
  auth:
    validate_with_local_oauth: true
    outbound_header_name: Authorization
    outbound_value_format: bearer
  routes:
    - id: my-upstream
      enabled: true
      path_prefix: /proxy/my-api
      upstream_base_url: http://127.0.0.1:9001
      auth_required: true
```

### Fields

| Field | Default | Description |
|---|---|---|
| `enabled` | `false` | Master switch for gateway proxying |
| `timeout_ms` | `2000` | Upstream request timeout |
| `max_body_bytes` | `1048576` | Max request/response body size |
| `auth.validate_with_local_oauth` | `true` | Must be `true` in v1 |
| `auth.outbound_header_name` | `Authorization` | Default outbound token header |
| `auth.outbound_value_format` | `bearer` | `bearer` or `raw` |
| `routes[].id` | — | Unique route identifier |
| `routes[].path_prefix` | — | Inbound path prefix to match |
| `routes[].upstream_base_url` | — | Absolute upstream URL |
| `routes[].auth_required` | `true` | Require valid token before proxying |
| `routes[].outbound_header_name` | — | Per-route header override |
| `routes[].outbound_value_format` | — | Per-route `bearer`/`raw` override |

### Environment variable overrides

```bash
O2MS_GATEWAY__ENABLED=true
O2MS_GATEWAY__TIMEOUT_MS=1500
```

## Running with gateway enabled

### 1. Start the mock server with gateway

Use the default config with gateway enabled via environment variable:

```bash
O2MS_GATEWAY__ENABLED=true \
  cargo run -p o2ms
```

Or create a dedicated config file (see `configs/mock-server.yaml` for the base).

### 2. Start an upstream API

For testing, run any HTTP server on the port matching your route's `upstream_base_url`. For example, a simple echo server:

```bash
python3 -m http.server 9001
```

### 3. Obtain a token

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8090/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=springboot-resource-server&scope=openid" | jq -r '.access_token')
```

### 4. Call the gateway route

```bash
curl http://127.0.0.1:8090/proxy/my-api/some/path \
  -H "Authorization: Bearer ${TOKEN}"
```

The gateway strips the `/proxy/my-api` prefix and forwards the request to `http://127.0.0.1:9001/some/path`.

### 5. Test auth rejection

```bash
curl http://127.0.0.1:8090/proxy/my-api/test
# → 401 {"error":"missing_bearer_token"}

curl http://127.0.0.1:8090/proxy/my-api/test \
  -H "Authorization: Bearer invalid-token"
# → 401 {"error":"invalid_bearer_token"}
```

## Error responses

| Status | Body | Cause |
|---|---|---|
| `401` | `{"error":"missing_bearer_token"}` | No token sent |
| `401` | `{"error":"invalid_bearer_token"}` | Token expired or malformed |
| `504` | `{"error":"upstream_timeout"}` | Upstream did not respond in time |
| `502` | `{"error":"upstream_unavailable"}` | Upstream connection refused |
| `502` | `{"error":"upstream_response_too_large"}` | Response exceeded `max_body_bytes` |

## Example topology

A typical local dev setup:

```
Client (curl / Java app)
    │
    ▼
Mock Server :8090
    ├── /token              ──► issues JWT
    ├── /.well-known/*      ──► OIDC discovery
    ├── /authorize          ──► browser login
    └── /proxy/users/*      ──► gateway ──► Upstream :9001
    └── /proxy/orders/*     ──► gateway ──► Upstream :9002
```

## Full config example

```yaml
gateway:
  enabled: true
  timeout_ms: 5000
  max_body_bytes: 2097152
  auth:
    validate_with_local_oauth: true
    outbound_header_name: X-Forwarded-Access-Token
    outbound_value_format: raw
  routes:
    - id: users-api
      enabled: true
      path_prefix: /proxy/users
      upstream_base_url: http://127.0.0.1:9001
      auth_required: true
    - id: orders-api
      enabled: true
      path_prefix: /proxy/orders
      upstream_base_url: http://127.0.0.1:9002
      auth_required: true
      outbound_header_name: Authorization
      outbound_value_format: bearer
```