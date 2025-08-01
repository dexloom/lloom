# Release Process

This document outlines the release process for the Lloom project, including versioning strategy, release procedures, and distribution methods.

## Versioning Strategy

### Semantic Versioning

Lloom follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH
```

- **MAJOR**: Incompatible API changes
- **MINOR**: Backward-compatible functionality additions
- **PATCH**: Backward-compatible bug fixes

### Version Synchronization

All crates in the workspace maintain synchronized versions:

```toml
# workspace Cargo.toml
[workspace.package]
version = "0.1.0"

# crate Cargo.toml
[package]
version.workspace = true
```

### Pre-release Versions

For pre-releases:
- Alpha: `0.1.0-alpha.1`
- Beta: `0.1.0-beta.1`
- Release Candidate: `0.1.0-rc.1`

## Release Types

### Regular Releases

Scheduled releases every 6-8 weeks including:
- New features
- Bug fixes
- Performance improvements
- Documentation updates

### Hotfix Releases

Urgent releases for critical issues:
- Security vulnerabilities
- Data loss bugs
- Service disruptions

### LTS Releases

Long-term support releases:
- Every 6 months
- Supported for 1 year
- Security and critical fixes only

## Release Preparation

### 1. Feature Freeze

Two weeks before release:

```bash
# Create release branch
git checkout -b release/v0.2.0
git push origin release/v0.2.0

# No new features after this point
```

### 2. Version Bump

Update version numbers:

```bash
# Update workspace version
sed -i 's/version = "0.1.0"/version = "0.2.0"/' Cargo.toml

# Update lock file
cargo update -p lloom-core -p lloom-client -p lloom-executor -p lloom-validator

# Commit changes
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.2.0"
```

### 3. Changelog Update

Update `CHANGELOG.md`:

```markdown
# Changelog

## [0.2.0] - 2024-01-15

### Added
- Network autodiscovery via mDNS (#123)
- GPU memory management for executors (#145)
- Response streaming support (#167)

### Changed
- Improved request routing algorithm (#134)
- Updated to libp2p 0.53 (#156)

### Fixed
- Memory leak in request queue (#128)
- Signature verification edge case (#139)

### Security
- Updated dependencies for CVE-2024-1234 (#150)
```

### 4. Documentation Update

Ensure documentation is current:

```bash
# Update version in docs
find docs -name "*.md" -exec sed -i 's/0.1.0/0.2.0/g' {} \;

# Regenerate API docs
cargo doc --no-deps --all-features

# Build and review mdBook
cd docs
mdbook build
mdbook serve
```

### 5. Testing

Run comprehensive tests:

```bash
# Full test suite
cargo test --all-features --workspace

# Integration tests
cargo test --test '*'

# Benchmarks (check for regressions)
cargo bench -- --baseline v0.1.0

# Security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

### 6. Release Candidate

Build and test release candidate:

```bash
# Tag RC
git tag -a v0.2.0-rc.1 -m "Release candidate 1 for v0.2.0"
git push origin v0.2.0-rc.1

# Build release binaries
cargo build --release --all

# Test on multiple platforms
# Linux, macOS, Windows (via CI)
```

## Release Checklist

### Pre-release Checklist

- [ ] All planned features merged
- [ ] All tests passing
- [ ] No security vulnerabilities (`cargo audit`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version numbers bumped
- [ ] Release notes drafted

### Build Checklist

- [ ] Release builds compile without warnings
- [ ] Binary sizes are reasonable
- [ ] Performance benchmarks acceptable
- [ ] Cross-platform builds successful
- [ ] Docker images build successfully

### Quality Checklist

- [ ] Manual testing completed
- [ ] Upgrade path tested
- [ ] Backward compatibility verified
- [ ] API documentation complete
- [ ] User guide updated

## Release Procedure

### 1. Final Release Branch

```bash
# Ensure release branch is up to date
git checkout release/v0.2.0
git pull origin release/v0.2.0

# Run final checks
make release-checks
```

### 2. Create Release Tag

```bash
# Tag the release
git tag -a v0.2.0 -m "Release v0.2.0

Major features:
- Network autodiscovery
- GPU memory management
- Response streaming

See CHANGELOG.md for full details."

# Push tag
git push origin v0.2.0
```

### 3. Build Release Artifacts

#### Binary Releases

```bash
# Build for all platforms
make release-build

# Output structure:
# target/release/
# ├── lloom-client-v0.2.0-linux-amd64.tar.gz
# ├── lloom-client-v0.2.0-darwin-amd64.tar.gz
# ├── lloom-client-v0.2.0-windows-amd64.zip
# └── ... (executor and validator binaries)
```

#### Docker Images

```bash
# Build and tag Docker images
docker build -t lloom/client:0.2.0 -t lloom/client:latest -f docker/client.Dockerfile .
docker build -t lloom/executor:0.2.0 -t lloom/executor:latest -f docker/executor.Dockerfile .
docker build -t lloom/validator:0.2.0 -t lloom/validator:latest -f docker/validator.Dockerfile .

# Push to registry
docker push lloom/client:0.2.0
docker push lloom/client:latest
# ... push all images
```

### 4. GitHub Release

Create release on GitHub:

```bash
# Using GitHub CLI
gh release create v0.2.0 \
  --title "Lloom v0.2.0" \
  --notes-file RELEASE_NOTES.md \
  --draft

# Upload artifacts
gh release upload v0.2.0 target/release/*.tar.gz target/release/*.zip

# Publish release
gh release edit v0.2.0 --draft=false
```

### 5. Publish to Crates.io

```bash
# Publish in dependency order
cd crates/lloom-core
cargo publish

cd ../lloom-client
cargo publish

cd ../lloom-executor
cargo publish

cd ../lloom-validator
cargo publish
```

### 6. Update Documentation

```bash
# Deploy documentation
cd docs
mdbook build
aws s3 sync book/ s3://docs.lloom.network/v0.2.0/
aws s3 sync book/ s3://docs.lloom.network/latest/

# Update version selector
# ... update website to include new version
```

## Post-Release

### 1. Merge Release Branch

```bash
# Merge to main
git checkout main
git merge --no-ff release/v0.2.0
git push origin main

# Delete release branch
git branch -d release/v0.2.0
git push origin --delete release/v0.2.0
```

### 2. Announcements

#### Blog Post

Write release blog post covering:
- Major features
- Performance improvements
- Breaking changes
- Upgrade instructions
- Acknowledgments

#### Social Media

- Twitter/X announcement
- Discord announcement
- Reddit post (r/rust, r/ethereum)
- Hacker News submission

#### Email Newsletter

Send to mailing list:
- Release highlights
- Migration guide
- Links to documentation

### 3. Update Dependencies

Projects depending on Lloom:

```toml
# Notify to update
[dependencies]
lloom-client = "0.2.0"
```

### 4. Monitor

Watch for issues after release:

- GitHub Issues
- Discord support channel
- Error tracking (Sentry)
- Performance metrics

## Hotfix Process

For critical fixes:

### 1. Create Hotfix Branch

```bash
# From the release tag
git checkout -b hotfix/v0.2.1 v0.2.0
```

### 2. Apply Fix

```bash
# Cherry-pick fix from main
git cherry-pick <commit-hash>

# Or apply fix directly
# ... make changes
git commit -m "fix: critical issue description"
```

### 3. Fast-track Release

```bash
# Bump patch version
# Update version to 0.2.1

# Tag and release
git tag -a v0.2.1 -m "Hotfix: <description>"
git push origin v0.2.1

# Build and publish immediately
```

## Automation

### CI/CD Pipeline

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Build release
        run: cargo build --release --all
        
      - name: Package binaries
        run: |
          # Platform-specific packaging
          
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        
  publish:
    needs: build
    steps:
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        
      - name: Publish to crates.io
        run: |
          cargo login ${{ secrets.CRATES_TOKEN }}
          make publish-crates
```

### Release Scripts

```bash
#!/bin/bash
# scripts/release.sh

VERSION=$1
RELEASE_TYPE=${2:-regular} # regular, hotfix, rc

# Validate version
if [[ ! $VERSION =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-.*)?$ ]]; then
    echo "Invalid version format"
    exit 1
fi

# Run release steps
./scripts/update-version.sh $VERSION
./scripts/run-tests.sh
./scripts/build-artifacts.sh $VERSION
./scripts/create-release.sh $VERSION $RELEASE_TYPE
```

## Security Releases

For security issues:

1. **Private Disclosure**: Security issues reported privately
2. **Patch Development**: Develop fix in private
3. **Coordinated Release**: 
   - Notify major users in advance
   - Publish advisory
   - Release patch
4. **CVE Assignment**: Request CVE if applicable

## Release Support

### Version Support Matrix

| Version | Status | Support Until | Notes |
|---------|---------|--------------|-------|
| 0.2.x | Current | - | Active development |
| 0.1.x | LTS | 2025-01-01 | Security fixes only |
| 0.0.x | EOL | - | No longer supported |

### Deprecation Policy

1. **Announce Deprecation**: At least one minor version notice
2. **Migration Guide**: Provide clear upgrade path
3. **Removal**: Remove in next major version

## Rollback Procedure

If critical issues found:

```bash
# Revert to previous version
docker pull lloom/executor:0.1.0
docker tag lloom/executor:0.1.0 lloom/executor:latest

# Yank from crates.io (within 72 hours)
cargo yank --vers 0.2.0 lloom-executor

# Communicate
# - GitHub issue
# - Discord announcement
# - Email to affected users
```

## Release Metrics

Track release success:

- Download counts
- GitHub stars/issues
- Discord activity
- Error rates post-release
- Performance metrics

## Lessons Learned

After each release:

1. **Retrospective Meeting**: What went well/poorly
2. **Process Updates**: Update this document
3. **Tooling Improvements**: Automate pain points
4. **Timeline Review**: Adjust future schedules