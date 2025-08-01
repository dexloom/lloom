# Quick Start

Get up and running with Lloom in minutes! This guide will walk you through setting up a local test network and making your first LLM request.

## Overview

In this quick start, you'll:
1. Start a local validator node for network bootstrap
2. Launch an executor with LMStudio backend
3. Make an LLM request as a client
4. Monitor the interaction

## Prerequisites

- Lloom binaries installed ([Installation Guide](./installation.md))
- LMStudio installed with at least one model loaded
- Basic familiarity with command line

## Step 1: Start the Validator Node

The validator acts as a bootstrap node for peer discovery:

```bash
# Generate validator identity (first time only)
lloom-validator generate-identity > validator-identity.json

# Start the validator
lloom-validator \
  --identity validator-identity.json \
  --listen /ip4/0.0.0.0/tcp/4001
```

You should see output like:
```
Starting Lloom Validator...
Local peer ID: 12D3KooWExample...
Listening on: /ip4/127.0.0.1/tcp/4001
Validator node is running. Press Ctrl+C to exit.
```

Keep this terminal open and note the listening address.

## Step 2: Configure and Start the Executor

### 2.1 Start LMStudio

1. Open LMStudio
2. Load a model (e.g., Llama 2 7B)
3. Go to "Local Server" tab
4. Click "Start Server"
5. Verify it's running at `http://localhost:1234`

### 2.2 Create Executor Configuration

Create `executor-config.toml`:

```toml
[network]
bootstrap_nodes = ["/ip4/127.0.0.1/tcp/4001"]

[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/api/v0"
supported_models = []  # Auto-discover
rate_limit = 100
```

### 2.3 Start the Executor

```bash
# Generate executor identity (first time only)
lloom-executor generate-identity > executor-identity.json

# Start the executor
lloom-executor \
  --identity executor-identity.json \
  --config executor-config.toml
```

You should see:
```
Starting Lloom Executor...
Discovered 1 models from LMStudio: ["llama-2-7b-chat.Q4_K_M.gguf"]
Connected to bootstrap node
Executor ready to process requests
```

## Step 3: Make Your First Request

Now let's send an LLM request as a client:

```bash
# Create a simple request
lloom-client request \
  --bootstrap /ip4/127.0.0.1/tcp/4001 \
  --model "llama-2-7b-chat.Q4_K_M.gguf" \
  --prompt "Explain what Rust is in one sentence." \
  --max-tokens 50
```

You'll see the request being processed:
```
Discovering executors...
Found executor: 12D3KooWExecutor... with model llama-2-7b-chat.Q4_K_M.gguf
Sending request...
Response received:

Rust is a systems programming language that emphasizes memory safety, 
concurrency, and performance without requiring a garbage collector.

Tokens used: 42
Model: llama-2-7b-chat.Q4_K_M.gguf
```

## Step 4: Understanding the Flow

Here's what just happened:

1. **Client** connected to the validator to discover executors
2. **Validator** provided information about available executors
3. **Client** found an executor with the requested model
4. **Client** sent a signed request directly to the executor
5. **Executor** processed the request using LMStudio
6. **Executor** returned a signed response with the result
7. **Client** verified the response and displayed it

## Step 5: Try More Examples

### Different Models

If you have multiple models in LMStudio:

```bash
# List available models
lloom-client discover-models --bootstrap /ip4/127.0.0.1/tcp/4001

# Request with a specific model
lloom-client request \
  --bootstrap /ip4/127.0.0.1/tcp/4001 \
  --model "mistral-7b-instruct.Q4_K_M.gguf" \
  --prompt "Write a haiku about peer-to-peer networks" \
  --max-tokens 50
```

### System Prompts

Add context with system prompts:

```bash
lloom-client request \
  --bootstrap /ip4/127.0.0.1/tcp/4001 \
  --model "llama-2-7b-chat.Q4_K_M.gguf" \
  --system-prompt "You are a helpful coding assistant specialized in Rust" \
  --prompt "How do I handle errors in Rust?" \
  --max-tokens 200
```

### Temperature Control

Adjust creativity with temperature:

```bash
# More creative (higher temperature)
lloom-client request \
  --bootstrap /ip4/127.0.0.1/tcp/4001 \
  --model "llama-2-7b-chat.Q4_K_M.gguf" \
  --prompt "Write a creative story opening" \
  --temperature 0.9 \
  --max-tokens 100

# More focused (lower temperature)
lloom-client request \
  --bootstrap /ip4/127.0.0.1/tcp/4001 \
  --model "llama-2-7b-chat.Q4_K_M.gguf" \
  --prompt "List the steps to install Rust" \
  --temperature 0.3 \
  --max-tokens 150
```

## Monitoring

### View Logs

Each component provides detailed logs:

```bash
# Set log level for more details
export RUST_LOG=debug

# Restart components to see debug logs
```

### Check Network Status

```bash
# See connected peers
lloom-client peers --bootstrap /ip4/127.0.0.1/tcp/4001

# Discover all executors
lloom-client discover-executors --bootstrap /ip4/127.0.0.1/tcp/4001
```

## Stopping the Network

To stop the test network:

1. Press `Ctrl+C` in each terminal
2. The nodes will gracefully shut down

## What's Next?

Congratulations! You've successfully:
- Set up a local Lloom network
- Made LLM requests through the P2P network
- Explored different request options

### Next Steps

1. **[Development Environment](./development-environment.md)**: Set up a full development environment with blockchain
2. **[Configuration Guide](./configuration.md)**: Learn about advanced configuration options
3. **[Running an Executor](../user-manual/executor.md)**: Set up a production executor
4. **[API Documentation](../api/client-library.md)**: Integrate Lloom into your applications

### Try These Challenges

1. Start multiple executors with different models
2. Use different LLM backends (OpenAI, custom)
3. Monitor resource usage during requests
4. Experiment with concurrent requests

## Troubleshooting Quick Start Issues

### Executor Can't Find Models
- Ensure LMStudio server is running
- Check the endpoint URL in config
- Try manually specifying models in config

### Connection Refused
- Verify all nodes are running
- Check firewall settings
- Ensure bootstrap addresses match

### No Executors Found
- Wait a few seconds for discovery
- Check executor connected successfully
- Verify model names match

For more help, see the full [Troubleshooting Guide](../user-manual/troubleshooting.md).