# Contributing

Thank you for your interest in contributing to Lloom! This guide will help you get started with contributing to the project.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct:

1. **Be Respectful**: Treat everyone with respect and kindness
2. **Be Inclusive**: Welcome people of all backgrounds and experience levels
3. **Be Professional**: Focus on what is best for the community
4. **Be Collaborative**: Work together to resolve conflicts

## How to Contribute

### Reporting Issues

Found a bug or have a feature request? Please open an issue:

1. **Check existing issues** first to avoid duplicates
2. **Use issue templates** when available
3. **Provide detailed information**:
   - Clear description of the problem
   - Steps to reproduce
   - Expected vs actual behavior
   - System information (OS, Rust version, etc.)
   - Relevant logs or error messages

### Suggesting Features

We welcome feature suggestions! Please:

1. **Open a discussion** first for major features
2. **Explain the use case** and benefits
3. **Consider implementation complexity**
4. **Be open to feedback** and alternative approaches

### Submitting Code

#### Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork**:
   ```bash
   git clone https://github.com/YOUR_USERNAME/lloom.git
   cd lloom
   ```

3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/dexloom/lloom.git
   ```

4. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

#### Development Process

1. **Keep your fork updated**:
   ```bash
   git fetch upstream
   git checkout main
   git merge upstream/main
   ```

2. **Make your changes**:
   - Write clean, documented code
   - Follow the coding standards
   - Add tests for new functionality
   - Update documentation as needed

3. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add new feature
   
   - Detailed description of what changed
   - Why the change was made
   - Any breaking changes"
   ```

4. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Create a Pull Request**:
   - Use a clear, descriptive title
   - Fill out the PR template
   - Link related issues
   - Request reviews from maintainers

## Development Guidelines

### Code Style

We use Rust's standard formatting and linting tools:

```bash
# Format code
cargo fmt

# Check linting
cargo clippy -- -D warnings

# Run both before committing
make pre-commit
```

#### Style Guidelines

1. **Naming Conventions**:
   ```rust
   // Modules: snake_case
   mod network_protocol;
   
   // Types: PascalCase
   struct LlmRequest;
   enum MessageType;
   
   // Functions: snake_case
   fn process_request() {}
   
   // Constants: SCREAMING_SNAKE_CASE
   const MAX_RETRIES: u32 = 3;
   ```

2. **Documentation**:
   ```rust
   /// Brief description of the function.
   ///
   /// More detailed explanation if needed.
   ///
   /// # Arguments
   ///
   /// * `param` - Description of parameter
   ///
   /// # Returns
   ///
   /// Description of return value
   ///
   /// # Examples
   ///
   /// ```
   /// let result = function_name(param);
   /// assert_eq!(result, expected);
   /// ```
   pub fn function_name(param: Type) -> Result<ReturnType> {
       // Implementation
   }
   ```

3. **Error Handling**:
   ```rust
   // Use Result for fallible operations
   fn may_fail() -> Result<Data, Error> {
       // Prefer ? operator
       let data = risky_operation()?;
       Ok(process(data))
   }
   
   // Custom error types
   #[derive(Debug, thiserror::Error)]
   pub enum ProcessingError {
       #[error("Invalid input: {0}")]
       InvalidInput(String),
       
       #[error("Network error: {0}")]
       Network(#[from] NetworkError),
   }
   ```

### Testing Requirements

All contributions must include appropriate tests:

1. **Unit Tests**: For individual functions and modules
2. **Integration Tests**: For feature interactions
3. **Documentation Tests**: For example code in docs

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_functionality() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = new_function(input);
        
        // Assert
        assert_eq!(result, expected_output());
    }
}
```

Run tests before submitting:

```bash
# Run all tests
cargo test

# Run with coverage
cargo tarpaulin --out Html

# Run specific test
cargo test test_new_functionality
```

### Documentation

Update documentation for any changes:

1. **Code Documentation**: Doc comments for public APIs
2. **README**: Update if adding major features
3. **User Guide**: Update relevant sections
4. **API Docs**: Ensure `cargo doc` generates correctly

```bash
# Generate and view documentation
cargo doc --open

# Check documentation
cargo doc --no-deps --document-private-items
```

### Commit Messages

Follow conventional commits format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples**:
```
feat(network): add automatic peer discovery

Implement mDNS-based peer discovery for local networks.
This allows nodes to automatically find each other without
manual configuration.

Closes #123
```

```
fix(executor): prevent memory leak in request processing

The request queue was not properly cleaning up completed
requests, causing memory usage to grow over time.

Fixes #456
```

### Pull Request Process

1. **Before Opening PR**:
   - Ensure all tests pass
   - Run formatting and linting
   - Update documentation
   - Rebase on latest main

2. **PR Description**:
   - Clear summary of changes
   - Link related issues
   - List breaking changes
   - Include testing instructions

3. **Review Process**:
   - Address reviewer feedback promptly
   - Keep PR focused and reasonably sized
   - Split large changes into multiple PRs

4. **After Approval**:
   - Squash commits if requested
   - Ensure CI passes
   - Maintainer will merge

## Project Architecture

### Crate Structure

```
lloom/
├── crates/
│   ├── lloom-core/      # Core functionality
│   ├── lloom-client/    # Client implementation
│   ├── lloom-executor/  # Executor implementation
│   └── lloom-validator/ # Validator implementation
├── solidity/            # Smart contracts
├── docs/               # Documentation
└── tests/              # Integration tests
```

### Key Design Principles

1. **Modularity**: Separate concerns into different crates
2. **Async-First**: Use async/await throughout
3. **Error Handling**: Explicit error types with context
4. **Performance**: Optimize hot paths, profile regularly
5. **Security**: Validate all inputs, sign critical messages

### Dependencies

When adding dependencies:

1. **Justify the need** for new dependencies
2. **Check license compatibility** (MIT/Apache 2.0 preferred)
3. **Prefer well-maintained** crates
4. **Avoid duplicate functionality**
5. **Keep dependency tree clean**

## Development Setup

### Required Tools

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Development tools
cargo install cargo-watch cargo-edit cargo-outdated

# Code quality tools
cargo install cargo-clippy cargo-fmt

# Testing tools
cargo install cargo-tarpaulin cargo-nextest
```

### Recommended IDE Setup

#### VS Code

Install extensions:
- rust-analyzer
- Even Better TOML
- crates
- Error Lens

Settings:
```json
{
    "rust-analyzer.checkOnSave.command": "clippy",
    "editor.formatOnSave": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    }
}
```

#### RustRover/IntelliJ

1. Install Rust plugin
2. Configure external linter to use clippy
3. Enable format on save

### Building and Running

```bash
# Build all crates
cargo build --all

# Run specific binary
cargo run --bin lloom-executor

# Watch for changes
cargo watch -x build -x test

# Run with debug logging
RUST_LOG=debug cargo run
```

## Communication Channels

### Discord

Join our Discord server for:
- Quick questions
- Development discussions
- Community support

### GitHub Discussions

Use GitHub Discussions for:
- Feature proposals
- Architecture discussions
- General questions

### Issues

Use GitHub Issues for:
- Bug reports
- Feature requests
- Task tracking

## Release Process

### Version Numbering

We follow Semantic Versioning (SemVer):
- MAJOR: Breaking changes
- MINOR: New features (backward compatible)
- PATCH: Bug fixes (backward compatible)

### Release Checklist

1. Update version numbers in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run full test suite
4. Update documentation
5. Create release PR
6. Tag release after merge
7. Publish to crates.io

## Getting Help

### For Contributors

- Check the documentation first
- Search existing issues and discussions
- Ask in Discord #development channel
- Tag maintainers for complex issues

### For Maintainers

Maintainers should:
- Respond to issues within 48 hours
- Review PRs within one week
- Provide constructive feedback
- Help new contributors get started

## Recognition

We value all contributions:
- Code contributions
- Documentation improvements
- Bug reports and testing
- Community support
- Translations

Contributors are recognized in:
- Release notes
- CONTRIBUTORS.md file
- Project documentation

## License

By contributing to Lloom, you agree that your contributions will be licensed under the MIT License.

## Thank You!

Your contributions make Lloom better for everyone. We appreciate your time and effort in improving the project!