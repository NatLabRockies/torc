# OpenAPI Workflow

Torc uses a Rust-owned OpenAPI workflow. The server emits the authoritative spec, and the Rust,
Python, and Julia clients are generated from that emitted contract.

## Current State

- `openapi.codegen.yaml`: full API spec emitted from hand-owned Rust code.
- `openapi.yaml`: checked-in distribution artifact that can now be refreshed from Rust.
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
