# Quick design

The mock server is a thin Rust wrapper around `oauth2-test-server`.

## Runtime flow

1. The binary loads YAML configuration.
2. The wrapper validates the configuration and builds the effective app config.
3. The wrapper maps config values into the upstream `oauth2-test-server` issuer config.
4. The wrapper owns the outer Axum router and can add local helper endpoints such as `/health`.
5. The upstream router handles the OAuth/OIDC endpoints.

## Current module layout

- `crate/src/app` - startup and wrapper state
- `crate/src/config` - YAML model, loader, and validation
- `crate/src/upstream` - integration adapter and patch strategy
- `crate/src/http` - wrapper router and helper endpoints
- `crate/src/registry` - planned client/user registry logic
- `crate/src/claims` - planned claim merge/protection logic

## Why the wrapper exists

The upstream crate already provides the OAuth/OIDC engine. This repository adds the higher-level behavior needed for local integration testing:

- YAML-driven configuration
- client and user modeling
- future custom JWT claims support
- future token-header response customization
- Spring Boot-oriented examples and documentation
