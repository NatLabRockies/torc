# Server Layout

The server is no longer organized around generated routing code.

Current ownership is:

- `api_contract.rs`
  - server-owned contract traits for the live transport layer
  - `TransportApi` is the composed transport-facing surface
- `response_types.rs`
  - owned facade that groups transport response enums by domain
  - callers should prefer this over importing `api_responses.rs` directly
- `api_responses.rs`
  - owned transport response enums used by the server contract and HTTP mapping layer
- `http_transport.rs`
  - live HTTP transport shell and route interception/dispatch
- `http_server.rs`
  - `Server<C>` plus top-level transport delegation
- `http_server/`
  - transport-facing server methods split by domain plus non-transport support modules
- `http_transport/`
  - HTTP handlers split by the same domain groupings as `http_server/`
- `transport_types/`
  - request/auth/context support types for server transport code
- `api/`
  - lower-level domain implementations that talk to the database

When adding a new endpoint:

1. Update the live Rust-owned OpenAPI surface if the contract changes.
2. Add or update the transport handler in `http_transport.rs`.
3. Put transport-specific implementation in the matching `http_server/*.rs` domain module.
4. Reuse the lower-level domain API implementation in `src/server/api/` where possible.
5. Import response enums through `response_types.rs`, not `api_responses.rs`, unless you are editing
   the underlying transport enum definitions.

Validation for server transport work:

```bash
cargo nextest run --lib --no-default-features --features server-bin -E 'test(http_transport)'
bash api/check_openapi_codegen_parity.sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=1 cargo clippy --all --all-targets --all-features -- -D warnings
```
