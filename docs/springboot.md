# Spring Boot integration

This repository includes a **Maven-based Spring Boot 4.0.6** example under:

```text
examples/springboot-v4-resource-server/
```

The example supports both:

1. a **resource-server** path that validates bearer JWTs from the mock server
2. a **browser login** path that uses `authorization_code` against the mock server

## What this example covers

1. Spring Boot 4 resource-server configuration with `issuer-uri`
2. Spring Boot OAuth2 client configuration for browser login
3. JWT validation against the mock server's discovery and JWKS endpoints
4. Reading custom claims such as `roles` and `authorizations` from the validated JWT

## Run the mock server

Use the Spring-oriented mock config:

```bash
cargo run -p oauth2-mock-server -- --config configs/mock-server.springboot.yaml
```

You can also override the bind port or issuer URL from the CLI:

```bash
cargo run -p oauth2-mock-server -- \
  --config configs/mock-server.springboot.yaml \
  --bind-port 9191 \
  --issuer-base-url http://127.0.0.1:9191
```

## Run the Spring Boot example

From the example directory:

```bash
cd examples/springboot-v4-resource-server
MOCK_ISSUER_URI=http://127.0.0.1:8090 mvn spring-boot:run
```

Or from the repository root:

```bash
MOCK_ISSUER_URI=http://127.0.0.1:8090 \
  mvn -f examples/springboot-v4-resource-server/pom.xml spring-boot:run
```

The example app listens on `http://127.0.0.1:8081`.

## Request a token from the mock server

With the Spring Boot sample config, the preloaded API client ID is `springboot-resource-server`.

```bash
curl -X POST http://127.0.0.1:8090/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=springboot-resource-server&scope=openid profile email"
```

The response keeps the normal JSON body and also emits:

```http
Authorization: Bearer <access-token>
```

## Call the Spring Boot API

If you want the token from the JSON body:

```bash
TOKEN="$(curl -s -X POST http://127.0.0.1:8090/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=springboot-resource-server&scope=openid profile email" | jq -r '.access_token')"

curl http://127.0.0.1:8081/api/me -H "Authorization: Bearer ${TOKEN}"
```

If you want the token from the response header instead, read the `Authorization` header from the token endpoint response and forward it as-is to the resource server.

## Simulate a user flow

The same mock config also preloads a browser-login client:

- client ID: `springboot-web-client`
- client secret: `springboot-secret`
- redirect URI: `http://127.0.0.1:8081/login/oauth2/code/mock`

The mock server uses the **first enabled user** in `users:` as the simulated logged-in user. In `configs/mock-server.springboot.yaml`, that is currently `demo-user`.

The Spring Boot sample config also enables the mock server's simple authorization user picker, so the `/authorize` step shows a small HTML page where you can type or choose one of the enabled YAML `user_id` values.

To exercise the browser login flow:

1. Start the mock server with `configs/mock-server.springboot.yaml`
2. Start the Spring Boot example
3. Open the protected browser endpoint:

```text
http://127.0.0.1:8081/login/me
```

Because `/login/me` is protected, Spring Boot redirects to the mock server `/authorize`, the mock server shows the picker page, and after you choose a user Spring Boot completes the login callback automatically.

On the picker page, try one of these sample user IDs:

- `demo-user`
- `support-user`

After login, inspect the logged-in user session at:

```text
http://127.0.0.1:8081/login/me
```

That endpoint returns the OIDC user claims seen by the Spring Boot app.

Useful endpoints in the example app:

- `/` - quick links and entry points
- `/login/me` - protected browser resource that triggers login and then shows the OIDC user session
- `/oauth2/authorization/mock` - optional direct login entry point
- `/api/me` - protected API endpoint for bearer-token validation
- `/api/ping` - simple protected API endpoint for quickly testing whether a bearer token is accepted

## Important note about custom token headers

The Spring Boot example is a **resource server**, so it does not consume `/token` responses directly. It only validates bearer JWTs that your client sends to it.

That means:

- custom token-response headers are useful for the client or test harness obtaining the token
- the Spring resource server still expects the standard inbound request header:

```http
Authorization: Bearer <jwt>
```

For the browser login flow, Spring Boot exchanges the authorization code server-side, so you do not manually call `/token` yourself.
