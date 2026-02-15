# Testing the Release Workflow

## Quick Start

### 1. Trigger a test build

```bash
# Create and push a test tag
git tag v0.7.0-test1
git push origin v0.7.0-test1
```

### 2. Monitor the build

Go to: https://github.com/NatLabRockies/torc/actions/workflows/release.yml

You should see 4 build jobs running in parallel:

- ✅ Build aarch64-apple-darwin (macOS Apple Silicon)
- ⚠️ Build x86_64-unknown-linux-musl (Linux static)
- ✅ Build x86_64-unknown-linux-gnu (Linux glibc)
- ✅ Build x86_64-pc-windows-msvc (Windows)

### 3. Check for failures

**If musl build fails with OpenSSL errors:**

```bash
# Apply the fix
git apply .github/openssl-musl-fix.patch

# Or manually add to torc-server/Cargo.toml:
[target.'cfg(target_env = "musl")'.dependencies]
openssl = { workspace = true, features = ["vendored"] }

# Commit and push new tag
git add torc-server/Cargo.toml
git commit -m "Fix musl builds with vendored OpenSSL"
git tag v0.7.0-test2
git push origin v0.7.0-test2
```

**If kaleido download fails:**

- This is usually a transient network issue
- Re-run the failed job from GitHub Actions UI

### 4. Download and test artifacts

Once builds succeed, artifacts are available for 7 days:

```bash
# Download from GitHub Actions UI
# Or use gh CLI:
gh run download <run-id>

# Extract and test
tar xzf torc-aarch64-apple-darwin.tar.gz
./torc --version
./torc-server --version
./torc-slurm-job-runner --version
```

### 5. Test on different Linux distributions

**Test musl binary on Alpine:**

```bash
docker run -it --rm -v $(pwd):/workspace alpine:latest sh
cd /workspace
./torc --version
```

**Test glibc binary on Ubuntu 20.04:**

```bash
docker run -it --rm -v $(pwd):/workspace ubuntu:20.04 sh
cd /workspace
apt-get update && apt-get install -y ca-certificates
./torc --version
```

**Test glibc binary on Ubuntu 24.04:**

```bash
docker run -it --rm -v $(pwd):/workspace ubuntu:24.04 sh
cd /workspace
./torc --version
```

### 6. Create actual release

Once testing is complete:

```bash
# Tag the release
git tag v0.7.0
git push origin v0.7.0

# Workflow will create a DRAFT release
# Go to GitHub releases and:
# 1. Review the release notes
# 2. Edit description if needed
# 3. Click "Publish release"
```

## Troubleshooting

### Build takes too long / times out

GitHub Actions has a 6-hour job limit. Our builds should complete in ~10-30 minutes per platform.

If timing out:

- Check if dependencies are being cached (cache hit logs)
- Consider removing less important targets

### Wrong binaries in release

Check the glob patterns in `create-release` job:

```yaml
files: |
  artifacts/torc-aarch64-apple-darwin/*.tar.gz
  artifacts/torc-x86_64-unknown-linux-musl/*.tar.gz
  artifacts/torc-x86_64-unknown-linux-gnu/*.tar.gz
  artifacts/torc-x86_64-pc-windows-msvc/*.zip
```

### Release not created

- Check that you pushed a tag starting with `v` (e.g., `v0.7.0`)
- The `create-release` job only runs when `startsWith(github.ref, 'refs/tags/')`
- For manual workflow runs, artifacts are created but no release

### Testing without creating a release

Use manual trigger:

1. Go to Actions → "Build Release Binaries" → "Run workflow"
2. Leave tag name empty or use "test"
3. Artifacts will be created but no release

## Cleanup

Delete test tags when done:

```bash
# Delete local tags
git tag -d v0.7.0-test1 v0.7.0-test2

# Delete remote tags
git push origin --delete v0.7.0-test1 v0.7.0-test2

# Delete releases from GitHub UI if any were created
```

## Performance Tips

### Faster builds with better caching

The workflow already caches:

- `~/.cargo/registry` (downloaded crates)
- `~/.cargo/git` (git dependencies)
- `target` (compiled artifacts)

Cache is keyed by:

- OS and target triple
- Cargo.lock hash

To bust cache (if needed):

- Update Cargo.lock: `cargo update`
- Or manually delete cache from GitHub UI

### Reduce build targets for testing

Edit `.github/workflows/release.yml` temporarily:

```yaml
matrix:
  include:
    # Only build macOS for quick test
    - os: macos-14
      target: aarch64-apple-darwin
      use_cross: false
```

## Next Steps

After successful test:

1. Document installation instructions for users
2. Add checksums (sha256) to release notes
3. Consider setting up a release schedule
4. Add auto-update checking to CLI
