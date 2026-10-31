# Release Build Guide

This document explains how to use the automated release build system for Torc.

## Overview

The release workflow builds binaries for:

- **macOS**: Apple Silicon (aarch64)
- **Linux**:
  - x86_64-musl (static, works on all distros)
  - x86_64-gnu (glibc, Ubuntu 20.04+ compatible)
- **Windows**: x86_64

## Binaries Produced

Each platform build produces three binaries:

1. `torc` - Unified CLI with all features
2. `torc-server` - Standalone server
3. `torc-slurm-job-runner` - Slurm job runner

## Triggering a Release

### Automatic (Recommended)

Push a version tag to trigger the build:

```bash
git tag v0.7.0
git push origin v0.7.0
```

This will:

1. Build binaries for all platforms
2. Create a draft GitHub release
3. Upload all binaries to the release

### Manual

You can also trigger builds manually from the GitHub Actions UI:

1. Go to Actions → "Build Release Binaries"
2. Click "Run workflow"
3. Optionally specify a tag name

Manual builds create artifacts but don't create a GitHub release.

## Linux Distribution Support

### Which Linux binary should users download?

**On x86_64:**

- Use `torc-x86_64-unknown-linux-musl.tar.gz`
- This is a fully static binary that works on any Linux distro
- No external dependencies required

**On ARM 64-bit (Raspberry Pi OS 64-bit, AWS Graviton, Ampere, etc.):**

- Use `torc-aarch64-unknown-linux-musl.tar.gz`
- Also fully static; requires a 64-bit OS (`uname -m` reports `aarch64`)
- 32-bit ARM (`armv7l`) is not built

## Adding More Platforms

### Additional Linux Versions

To support older distros, add entries to the matrix in `.github/workflows/release.yml`:

```yaml
# Ubuntu 18.04 (glibc 2.27) for older distros
- os: ubuntu-18.04
  target: x86_64-unknown-linux-gnu
  use_cross: false
```

### Intel macOS

To also build for Intel Macs, add:

```yaml
# macOS Intel
- os: macos-13
  target: x86_64-apple-darwin
  use_cross: false
```

## Troubleshooting

### Build fails with OpenSSL errors on Windows

The workflow installs OpenSSL via vcpkg. If it fails:

1. Check that vcpkg is available on the runner
2. Verify the OpenSSL environment variables are set correctly

### Build fails with musl linking errors

If you see linker errors with musl:

1. Ensure `cross` is being used (set `use_cross: true`)
2. Or ensure musl-tools are installed for native builds

### Binary size is too large

Release binaries include debug symbols. To reduce size:

1. Add strip step to workflow after build:
   ```yaml
   - name: Strip binaries (Unix)
     if: matrix.os != 'windows-latest'
     run: |
       strip target/${{ matrix.target }}/release/torc
       strip target/${{ matrix.target }}/release/torc-server
       strip target/${{ matrix.target }}/release/torc-slurm-job-runner
   ```

2. Or add to Cargo.toml:
   ```toml
   [profile.release]
   strip = true
   ```

## Testing Release Binaries

After downloading a release archive:

```bash
# Linux/macOS
tar xzf torc-x86_64-unknown-linux-musl.tar.gz
./torc --version
./torc-server --version

# Windows (PowerShell)
Expand-Archive torc-x86_64-pc-windows-msvc.zip
.\torc.exe --version
.\torc-server.exe --version
```

## Automation Tips

### Auto-publish releases

To automatically publish releases (instead of drafts), change in release.yml:

```yaml
draft: false  # Change from true
```

### Build on every push

To build binaries on every push (for testing), add to `on:` section:

```yaml
on:
  push:
    branches: [main]
  # ... rest of triggers
```

### Notification on failure

Add a notification step at the end of the build job:

```yaml
- name: Notify on failure
  if: failure()
  run: |
    # Add your notification logic (Slack, Discord, email, etc.)
```
