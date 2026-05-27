# Spring Boot integration

This repository includes a **Maven-based Spring Boot 4.0.6** example under:

```text
examples/springboot-v4-resource-server/
```

The example is a resource server that validates JWTs from the mock server through issuer discovery and JWKS.

## What this example covers

1. Spring Boot 4 resource-server configuration with `issuer-uri`
2. JWT validation against the mock server's discovery and JWKS endpoints
3. Reading custom claims such as `roles` and `authorizations` from the validated JWT

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

With the Spring Boot sample config, the preloaded client ID is `springboot-resource-server`.

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

## Important note about custom token headers

The Spring Boot example is a **resource server**, so it does not consume `/token` responses directly. It only validates bearer JWTs that your client sends to it.

That means:

- custom token-response headers are useful for the client or test harness obtaining the token
- the Spring resource server still expects the standard inbound request header:

```http
Authorization: Bearer <jwt>
```

If you later build a Spring OAuth client example, token-header customization will need client-side code instead of standard Spring Security autoconfiguration.
