# GitHub Actions CI/CD Documentation

This project uses GitHub Actions for continuous integration and deployment with several optimizations for Rust projects.

## Workflow Overview

### 1. CI - Lint & Build (`ci.yml`)
Runs on every push and pull request to ensure code quality.

**Features:**
- **sccache**: Distributed compiler cache for faster builds
- **Multi-platform builds**: Tests on Linux, macOS, and Windows
- **Comprehensive checks**: Format, clippy, documentation, typos
- **Security scanning**: cargo-audit and cargo-deny
- **Artifact caching**: Dependencies and build outputs

**Best Practices Used:**
- `SCCACHE_GHA_ENABLED` for GitHub Actions integration
- Matrix builds for parallel platform testing
- Separate caches for different purposes
- Cancel-in-progress for faster iteration

### 2. Tests (`tests.yml`)
Comprehensive test suite with database integration.

**Features:**
- Unit tests across all platforms
- Integration tests with real PostgreSQL and MySQL
- Documentation tests
- Code coverage with tarpaulin
- Service containers for databases

**Optimizations:**
- Parallel test execution where possible
- Test result caching
- Database service health checks

### 3. Release (`release.yml`)
Automated multi-platform binary releases.

**Features:**
- Builds for 8+ platform targets
- Cross-compilation using `cross`
- Automatic checksums (SHA256)
- Draft release creation
- Optional crates.io publishing

**Supported Platforms:**
- Linux: x86_64, x86_64-musl, aarch64, armv7
- macOS: x86_64, Apple Silicon (M1/M2)
- Windows: x86_64, i686

### 4. Alternative: cargo-dist (`release-dist.yml`)
Modern release automation using cargo-dist.

**Advantages:**
- Simpler configuration
- Automatic installer generation
- Built-in Homebrew formula updates
- Consistent artifact naming

## Caching Strategy

### sccache
- Caches compiled dependencies and build artifacts
- Shared across jobs in the same workflow
- Significantly reduces build times (often 50-70% improvement)

### Cargo Registry Cache
```yaml
path: |
  ~/.cargo/registry/index/
  ~/.cargo/registry/cache/
  ~/.cargo/git/db/
```
- Caches downloaded crates
- Keyed by Cargo.lock hash

### Build Cache
```yaml
path: target
key: ${{ runner.os }}-cargo-build-${{ hashFiles('**/Cargo.lock') }}
```
- Caches the entire target directory
- Platform-specific caches

## Performance Tips

1. **Use sccache**: Enabled by default in our workflows
2. **Precise Cache Keys**: Include Cargo.lock hash
3. **Matrix Builds**: Parallelize platform builds
4. **Cancel Previous Runs**: Avoid redundant builds
5. **Cross-compilation**: Use `cross` for Linux targets

## Release Process

### Manual Release
1. Create a tag: `git tag v1.0.0`
2. Push the tag: `git push origin v1.0.0`
3. GitHub Actions will:
   - Create a draft release
   - Build binaries for all platforms
   - Upload binaries with checksums
   - Optionally publish to crates.io

### Using cargo-dist
1. Install: `cargo install cargo-dist`
2. Init: `cargo dist init`
3. Create tag: `git tag dist-v1.0.0`
4. Push: `git push origin dist-v1.0.0`

## Environment Variables

### Required Secrets
- `CARGO_REGISTRY_TOKEN`: For crates.io publishing
- `CODECOV_TOKEN`: For coverage reporting (optional)

### Build Environment
- `CARGO_TERM_COLOR`: always
- `RUST_BACKTRACE`: 1
- `SCCACHE_GHA_ENABLED`: true
- `RUSTC_WRAPPER`: sccache

## Maintenance

### Updating Dependencies
```bash
# Update workflow actions
# Check for new versions at:
# - https://github.com/actions/checkout/releases
# - https://github.com/dtolnay/rust-toolchain/releases
# - https://github.com/mozilla-actions/sccache-action/releases
```

### Adding New Platforms
1. Add to the matrix in `release.yml`
2. Test cross-compilation locally
3. Update cargo-dist targets if using

### Debugging Builds
- Check sccache stats: Add step with `sccache --show-stats`
- Enable verbose logging: `RUST_LOG=debug`
- Use `act` for local testing: `act -j build`

## Cost Optimization

1. **Use buildjet for ARM builds**: Faster and cheaper than GitHub runners
2. **Cancel redundant workflows**: Implemented via concurrency groups
3. **Fail fast on critical jobs**: Security and format checks first
4. **Conditional steps**: Skip unnecessary work

## Security

- cargo-audit runs on every push
- cargo-deny checks licenses and vulnerabilities
- Dependabot configured for automatic updates
- SARIF upload for security results