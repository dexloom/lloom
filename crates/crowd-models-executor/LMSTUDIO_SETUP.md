# LMStudio Integration Guide

This guide explains how to set up and use LMStudio as a backend for the Crowd Models Executor.

## Prerequisites

1. **Install LMStudio**: Download and install LMStudio from [lmstudio.ai](https://lmstudio.ai)
2. **Load a Model**: Download and load a model in LMStudio (e.g., Llama 2, Mistral, etc.)
3. **Enable Local Server**: In LMStudio, go to the "Local Server" tab and start the server

## Configuration

### 1. Basic Configuration

Add the following configuration to your `config.toml`:

```toml
[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/api/v0"
# No API key needed for local LMStudio
supported_models = []  # Leave empty for auto-discovery
rate_limit = 100      # Higher limit for local processing
```

### 2. Custom Configuration

```toml
[[llm_backends]]
name = "lmstudio"
endpoint = "http://localhost:1234/api/v0"
supported_models = ["llama-2-7b-chat", "mistral-7b-instruct"]  # Specify exact models
rate_limit = 50
```

## Features

### Automatic Model Discovery
The executor automatically discovers available models from your running LMStudio instance:

```
INFO: Discovered 2 models from LMStudio: ["llama-2-7b-chat", "mistral-7b-instruct"]
INFO: Updated lmstudio backend with discovered models
```

### Enhanced Performance Metrics
LMStudio provides detailed performance information:

```
INFO: LLM request completed: 156 tokens used, 12.34 tokens/sec, 0.245s to first token, architecture: llama
```

### Model Information
Get detailed information about the model being used:
- Architecture (e.g., "llama", "mistral")
- Model size and parameters
- Performance statistics

## Usage

### 1. Start LMStudio
- Launch LMStudio
- Load your desired model
- Go to "Local Server" tab
- Click "Start Server" (default: http://localhost:1234)

### 2. Configure Executor
Update your `config.toml` with the LMStudio backend configuration.

### 3. Run Executor
```bash
cargo run --bin crowd-models-executor -- --config config.toml
```

The executor will:
1. Detect the LMStudio backend
2. Auto-discover available models
3. Start accepting requests

## Troubleshooting

### Connection Issues
- **Error**: `Failed to fetch models from LMStudio: 404`
  - **Solution**: Make sure LMStudio's local server is running
  - **Check**: Visit http://localhost:1234/api/v0/models in your browser

- **Error**: `Model discovery failed: Connection refused`
  - **Solution**: Verify LMStudio is running and the server is started
  - **Check**: Ensure no other services are using port 1234

### Model Issues
- **Warning**: `No models discovered from LMStudio backend`
  - **Solution**: Load a model in LMStudio before starting the executor
  - **Check**: Models tab in LMStudio should show at least one loaded model

### Performance Issues
- **Slow responses**: Check if your model is appropriate for your hardware
- **High memory usage**: Consider using smaller models or quantized versions

## Benefits of LMStudio Backend

1. **No API Costs**: Run models locally without per-token charges
2. **Privacy**: All inference happens locally on your machine
3. **Performance Metrics**: Detailed stats including tokens/sec and latency
4. **Model Flexibility**: Use any model supported by LMStudio
5. **Auto-Discovery**: Automatically finds loaded models
6. **Enhanced Logging**: Rich diagnostic information

## Example Output

```
🚀 Starting Crowd Models Executor...
INFO: Initialized LLM client for backend: lmstudio with models: ["llama-2-7b-chat"]
INFO: LLM request completed: 89 tokens used, 15.67 tokens/sec, 0.123s to first token, architecture: llama
```

## Supported Models

LMStudio supports a wide variety of models. Popular choices include:
- Llama 2 (7B, 13B, 70B)
- Mistral (7B variants)
- Code Llama
- And many others from Hugging Face

The executor will automatically detect and work with any model you have loaded in LMStudio.