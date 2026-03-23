# OpenAPI Workflow

Torc uses a Rust-owned OpenAPI workflow. The server emits the authoritative spec, and the Rust,
Python, and Julia clients are generated from that emitted contract.

## Current State

- `openapi.codegen.yaml`: full API spec emitted from hand-owned Rust code.
- `openapi.yaml`: checked-in distribution artifact that can now be refreshed from Rust.
- `openapi-generator-templates/rust/`: checked-in Rust OpenAPI generator template overrides used
  during regeneration.
- `sync_openapi.sh`: preferred entrypoint for emit/check/promote/client regeneration.

## Preferred Commands

Check that the checked-in specs match the Rust-emitted contract:

```bash
cd api
bash sync_openapi.sh check
```

Regenerate Rust, Python, and Julia clients from the checked-in contract artifact:

```bash
cd api
bash sync_openapi.sh clients
```

Regenerate Rust, Python, and Julia clients directly from the Rust-emitted spec without rewriting
`openapi.yaml`:

```bash
cd api
bash sync_openapi.sh clients --use-rust-spec
```

Promote the Rust-emitted spec into the checked-in artifact and regenerate clients from it:

```bash
cd api
bash sync_openapi.sh all --promote
```

Emit only the code-first scaffold:

```bash
cd api
bash sync_openapi.sh emit
```

Build both checked-in spec artifacts from Rust and regenerate clients:

```bash
cd api
bash sync_openapi.sh all --promote
```

## Developer Workflow

Emit the Rust-owned spec without touching the checked-in artifact:

```bash
cd api
bash sync_openapi.sh emit
```

Verify that `api/openapi.codegen.yaml` and `api/openapi.yaml` both match the Rust-emitted spec:

```bash
cd api
bash sync_openapi.sh check
```

Regenerate Rust, Python, and Julia clients from the checked-in contract:

```bash
cd api
bash sync_openapi.sh clients
```

Regenerate Rust, Python, and Julia clients from the Rust-emitted spec before promotion:

```bash
cd api
bash sync_openapi.sh clients --use-rust-spec
```

## Workflow Rules

1. Add or change API endpoints in the Rust-owned server/OpenAPI code.
2. Emit `openapi.codegen.yaml` from Rust and keep parity with `openapi.yaml`.
3. Promote the Rust spec into `openapi.yaml` with `bash sync_openapi.sh all --promote` when ready.
4. Generate Rust, Python, and Julia clients from the emitted spec instead of hand-editing client
   bindings.
5. Keep `api/openapi-generator-templates/` checked in because it is a required input to Rust client
   regeneration, not a local-only artifact.

## Rust Client Generation Details

The Rust client under `src/client/apis/` is generated from the checked-in OpenAPI spec, but it does
not own a second hand-maintained Rust model layer.

- `api/regenerate_rust_client.sh`
  - Generates a Rust client into a temporary directory from the selected spec.
  - Passes `api/openapi-generator-templates/rust/` to OpenAPI Generator with `-t`.
  - Copies only the generated grouped `*_api.rs` modules into `src/client/apis/`.
- `api/openapi-generator-templates/rust/`
  - Holds Torc's checked-in Rust generator customizations.
  - Makes generated request modules use `crate::models` instead of generated model files.
  - Applies repo-specific request behavior such as the shared auth hook.

The canonical Rust API model surface remains in `src/models.rs`. Generated Rust API modules are
downstream plumbing over that shared model layer, not an independent source of truth.
