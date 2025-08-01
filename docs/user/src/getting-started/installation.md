# Installation

This guide will help you install Lloom and its dependencies on your system.

## Prerequisites

Before installing Lloom, ensure you have the following prerequisites:

### System Requirements

- **Operating System**: Linux, macOS, or Windows (with WSL2)
- **RAM**: Minimum 8GB, 16GB recommended
- **Storage**: At least 20GB free space
- **Network**: Stable internet connection

### Required Software

#### Rust Toolchain

Lloom is built with Rust and requires the Rust toolchain:

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow the on-screen instructions, then reload your shell
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Foundry (for Smart Contracts)

Foundry is required for smart contract interaction:

```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash

# Follow the instructions to add foundryup to your PATH, then run:
foundryup

# Verify installation
forge --version
cast --version
```

#### Docker (Optional, for Development)

Docker is recommended for running the development environment:

```bash
# Install Docker Engine
# Visit https://docs.docker.com/engine/install/ for platform-specific instructions

# Verify installation
docker --version
docker compose version
```

## Installation Methods

### Method 1: Install from Source (Recommended)

Clone and build the Lloom project:

```bash
# Clone the repository
git clone https://github.com/lloom/lloom.git
cd lloom

# Build all components
cargo build --release

# The binaries will be available in target/release/:
# - lloom-client
# - lloom-executor
# - lloom-validator
```

### Method 2: Install Binaries

Install pre-built binaries directly:

```bash
# Install all Lloom binaries
cargo install --path crates/lloom-client
cargo install --path crates/lloom-executor
cargo install --path crates/lloom-validator

# Verify installation
lloom-client --version
lloom-executor --version
lloom-validator --version
```

### Method 3: Development Installation

For development, install with additional features:

```bash
# Clone with submodules
git clone --recursive https://github.com/lloom/lloom.git
cd lloom

# Install development dependencies
cargo install cargo-watch cargo-nextest

# Build in development mode
cargo build

# Run tests
cargo test
```

## Post-Installation Setup

### 1. Generate Identity

Each Lloom node needs a cryptographic identity:

```bash
# Generate a new identity
lloom-client generate-identity > ~/.lloom/identity.json

# Or use a specific private key
lloom-client generate-identity --private-key "your-private-key-hex" > ~/.lloom/identity.json
```

### 2. Configuration

Create a configuration file for your node:

```bash
# Create config directory
mkdir -p ~/.lloom

# Copy example configuration
cp crates/lloom-executor/config.toml.example ~/.lloom/config.toml

# Edit the configuration
nano ~/.lloom/config.toml
```

### 3. Verify Installation

Test your installation with a simple command:

```bash
# Check client
lloom-client --help

# Check executor
lloom-executor --help

# Check validator
lloom-validator --help
```

## Platform-Specific Notes

### Linux

Most Linux distributions work out of the box. For Ubuntu/Debian:

```bash
# Install build dependencies
sudo apt update
sudo apt install build-essential pkg-config libssl-dev
```

### macOS

On macOS, you might need to install additional tools:

```bash
# Install Xcode Command Line Tools
xcode-select --install

# If using Homebrew
brew install pkg-config
```

### Windows

Windows users should use WSL2 for the best experience:

1. Install WSL2 following [Microsoft's guide](https://docs.microsoft.com/en-us/windows/wsl/install)
2. Install Ubuntu or another Linux distribution
3. Follow the Linux installation instructions

## Environment Variables

Lloom supports configuration through environment variables:

```bash
# Set custom config path
export LLOOM_CONFIG_PATH="$HOME/.lloom/config.toml"

# Set identity file path
export LLOOM_IDENTITY_PATH="$HOME/.lloom/identity.json"

# Set log level
export RUST_LOG=info

# Add to your shell profile for persistence
echo 'export LLOOM_CONFIG_PATH="$HOME/.lloom/config.toml"' >> ~/.bashrc
```

## Troubleshooting

### Common Issues

**Problem**: `cargo: command not found`
- **Solution**: Ensure Rust is installed and `$HOME/.cargo/bin` is in your PATH

**Problem**: Build fails with OpenSSL errors
- **Solution**: Install OpenSSL development packages:
  ```bash
  # Ubuntu/Debian
  sudo apt install libssl-dev
  
  # macOS
  brew install openssl
  ```

**Problem**: `lloom-client: command not found` after installation
- **Solution**: Add Cargo's bin directory to your PATH:
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  ```

### Getting Help

If you encounter issues:

1. Check the [Troubleshooting Guide](../user-manual/troubleshooting.md)
2. Search existing [GitHub Issues](https://github.com/lloom/lloom/issues)
3. Join our community chat for real-time help

## Next Steps

Now that you have Lloom installed, proceed to:

- [Quick Start](./quick-start.md) - Run your first LLM request
- [Configuration](./configuration.md) - Customize your setup
- [Development Environment](./development-environment.md) - Set up local testing