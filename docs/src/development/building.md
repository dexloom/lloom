# Building from Source

This guide covers building the Lloom project from source code, including all dependencies, build configurations, and platform-specific instructions.

## Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows (with WSL2)
- **Memory**: Minimum 8GB RAM (16GB recommended for development)
- **Disk Space**: At least 10GB free space
- **CPU**: x86_64 or ARM64 architecture

### Required Tools

#### Rust Toolchain

Install Rust 1.75.0 or later:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Additional Dependencies

**Linux (Ubuntu/Debian)**:
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    clang \
    cmake
```

**macOS**:
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install protobuf cmake
```

**Windows (WSL2)**:
```bash
# Inside WSL2 Ubuntu
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev protobuf-compiler
```

### Optional Tools

For smart contract development:

```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Verify installation
forge --version
cast --version
anvil --version
```

## Building the Project

### Clone the Repository

```bash
git clone https://github.com/lloom/lloom.git
cd lloom

# If you have a specific branch or tag
git checkout <branch-or-tag>
```

### Build Commands

#### Development Build

Fast compilation with debug symbols:

```bash
cargo build
```

#### Release Build

Optimized build for production:

```bash
cargo build --release
```

#### Build Specific Components

Build individual crates:

```bash
# Build only the client
cargo build -p lloom-client

# Build only the executor
cargo build -p lloom-executor --release

# Build only the validator
cargo build -p lloom-validator --release
```

### Build Features

Enable or disable features:

```bash
# Build with all features
cargo build --all-features

# Build without default features
cargo build --no-default-features

# Build with specific features
cargo build --features "metrics,docker"
```

Available features:

| Feature | Description | Default |
|---------|-------------|---------|
| `metrics` | Prometheus metrics support | Yes |
| `docker` | Docker image building support | No |
| `cuda` | CUDA support for GPU acceleration | No |
| `test-utils` | Testing utilities | No |

## Platform-Specific Instructions

### Linux

#### GPU Support (NVIDIA)

For executor nodes with GPU support:

```bash
# Install CUDA toolkit
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.0-1_all.deb
sudo dpkg -i cuda-keyring_1.0-1_all.deb
sudo apt-get update
sudo apt-get -y install cuda

# Build with CUDA support
cargo build --release --features cuda
```

#### Systemd Service

Create systemd service files:

```bash
# Copy service files
sudo cp contrib/systemd/*.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload
```

### macOS

#### Apple Silicon (M1/M2)

Special considerations for ARM64:

```bash
# Ensure using native ARM64 Rust toolchain
rustup default stable-aarch64-apple-darwin

# Build with architecture-specific optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

#### Security Permissions

Grant network permissions:

```bash
# Add to firewall exceptions
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add $(pwd)/target/release/lloom-executor
```

### Cross-Compilation

#### Linux to Windows

```bash
# Add Windows target
rustup target add x86_64-pc-windows-gnu

# Install cross-compilation tools
sudo apt install mingw-w64

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu
```

#### Linux to macOS

Using [osxcross](https://github.com/tpoechtrager/osxcross):

```bash
# Set up osxcross (see their documentation)
# Then build
CC=o64-clang CXX=o64-clang++ cargo build --release --target x86_64-apple-darwin
```

## Build Optimization

### Release Optimizations

Configure `Cargo.toml` for maximum performance:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

### Binary Size Optimization

Reduce binary size:

```toml
[profile.release-small]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

Build with:
```bash
cargo build --profile release-small
```

### Compilation Time Optimization

Speed up development builds:

```toml
[profile.dev]
opt-level = 0
debug = 1  # Reduced debug info
incremental = true
```

Use sccache for caching:

```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
cargo build
```

## Docker Builds

### Build Docker Images

```bash
# Build all images
make docker-build

# Build specific image
docker build -t lloom-executor:latest -f docker/executor.Dockerfile .

# Multi-stage build for smaller images
docker build -t lloom-client:latest -f docker/client.multi.Dockerfile .
```

### Docker Compose Build

```bash
# Build all services
docker-compose build

# Build specific service
docker-compose build executor
```

## Build Verification

### Run Tests

Verify the build works correctly:

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p lloom-core

# Run integration tests
cargo test --test '*' --features test-utils
```

### Check Binary

Verify binary functionality:

```bash
# Check client
./target/release/lloom-client --version

# Check executor
./target/release/lloom-executor --version

# Run basic health check
./target/release/lloom-executor --health-check
```

## Troubleshooting

### Common Build Issues

#### OpenSSL Errors

```
error: failed to run custom build command for `openssl-sys`
```

**Solution**:
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev

# macOS
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```

#### Protobuf Errors

```
error: failed to run custom build command for `prost-build`
```

**Solution**:
```bash
# Ubuntu/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf
```

#### Linking Errors

```
error: linking with `cc` failed
```

**Solution**:
```bash
# Install build essentials
sudo apt-get install build-essential

# For cross-compilation, ensure correct linker
export CC=gcc
export CXX=g++
```

### Performance Issues

#### Slow Compilation

1. **Use mold linker**:
   ```bash
   cargo install mold
   export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
   ```

2. **Enable parallel compilation**:
   ```bash
   export CARGO_BUILD_JOBS=8  # Adjust based on CPU cores
   ```

3. **Use cargo-nextest for faster tests**:
   ```bash
   cargo install cargo-nextest
   cargo nextest run
   ```

#### Out of Memory

For systems with limited RAM:

```bash
# Limit parallel jobs
export CARGO_BUILD_JOBS=2

# Use swap file
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

## Development Setup

### IDE Configuration

#### VS Code

Install recommended extensions:
```json
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "tamasfe.even-better-toml",
        "serayuzgur.crates"
    ]
}
```

#### JetBrains RustRover

Configure build:
1. Open Settings → Build → Cargo
2. Set "External linter" to `clippy`
3. Enable "Format on save"

### Git Hooks

Install pre-commit hooks:

```bash
# Install pre-commit
pip install pre-commit

# Install hooks
pre-commit install

# Run manually
pre-commit run --all-files
```

## Continuous Integration

### GitHub Actions

The project uses GitHub Actions for CI:

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
      - uses: actions-rs/cargo@v1
        with:
          command: build
          args: --release --all-features
```

### Local CI

Run CI checks locally:

```bash
# Install act
brew install act  # macOS
# or
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash  # Linux

# Run CI locally
act -j build
```

## Build Artifacts

### Output Locations

Build outputs are located in:

```
target/
├── debug/          # Debug builds
│   ├── lloom-client
│   ├── lloom-executor
│   └── lloom-validator
├── release/        # Release builds
│   ├── lloom-client
│   ├── lloom-executor
│   └── lloom-validator
└── doc/           # Generated documentation
```

### Packaging

Create distribution packages:

```bash
# Create tarball
make dist

# Create Debian package
cargo install cargo-deb
cargo deb -p lloom-client

# Create RPM package
cargo install cargo-generate-rpm
cargo generate-rpm
```

## Next Steps

After building successfully:

1. [Configure your node](../getting-started/configuration.md)
2. [Run tests](./testing.md)
3. [Set up development environment](../getting-started/development-environment.md)
4. [Deploy your node](../getting-started/quick-start.md)