#!/usr/bin/env python3
"""
Mock LLM Server for testing the Crowd Models network.

This server mimics the OpenAI API format but returns mock responses
to allow testing without requiring actual API keys.
"""

import json
import time
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse
import argparse
import logging

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class MockLLMHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        """Handle POST requests to the chat completions endpoint."""
        
        # Parse the URL path
        parsed_path = urlparse(self.path)
        
        if parsed_path.path != '/chat/completions':
            self.send_error(404, "Not Found")
            return
        
        # Read the request body
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)
        
        try:
            request_data = json.loads(post_data.decode('utf-8'))
            logger.info(f"Received request for model: {request_data.get('model', 'unknown')}")
            logger.info(f"Prompt length: {len(request_data.get('messages', []))}")
            
            # Extract parameters
            model = request_data.get('model', 'gpt-3.5-turbo')
            messages = request_data.get('messages', [])
            max_tokens = request_data.get('max_tokens', 150)
            temperature = request_data.get('temperature', 0.7)
            
            # Generate mock response based on the prompt
            prompt_text = ""
            for msg in messages:
                if msg.get('role') == 'user':
                    prompt_text += msg.get('content', '')
            
            # Create a mock response
            mock_content = self.generate_mock_response(prompt_text, model)
            
            # Calculate mock token counts (rough estimation)
            prompt_tokens = len(prompt_text.split()) * 1.3  # Rough token estimation
            completion_tokens = len(mock_content.split()) * 1.3
            total_tokens = int(prompt_tokens + completion_tokens)
            
            # Build the response
            response = {
                "id": f"chatcmpl-{int(time.time())}",
                "object": "chat.completion",
                "created": int(time.time()),
                "model": model,
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": mock_content
                        },
                        "finish_reason": "stop"
                    }
                ],
                "usage": {
                    "prompt_tokens": int(prompt_tokens),
                    "completion_tokens": int(completion_tokens),
                    "total_tokens": total_tokens
                }
            }
            
            # Send response
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(response).encode('utf-8'))
            
            logger.info(f"Sent mock response with {total_tokens} tokens")
            
        except json.JSONDecodeError:
            self.send_error(400, "Invalid JSON")
        except Exception as e:
            logger.error(f"Error processing request: {e}")
            self.send_error(500, "Internal Server Error")
    
    def generate_mock_response(self, prompt, model):
        """Generate a mock response based on the prompt."""
        
        # Simple responses based on keywords
        prompt_lower = prompt.lower()
        
        if "hello" in prompt_lower or "hi" in prompt_lower:
            return "Hello! I'm a mock AI assistant running in the Crowd Models test environment. How can I help you today?"
        
        elif "what" in prompt_lower and "name" in prompt_lower:
            return f"I'm a mock version of {model}, running on the Crowd Models decentralized network for testing purposes."
        
        elif "code" in prompt_lower or "python" in prompt_lower:
            return """Here's a simple Python example:

```python
def hello_world():
    print("Hello from the Crowd Models network!")
    return "Success"

# This is a mock response for testing
hello_world()
```

This is a mock code response generated for testing the Crowd Models P2P network."""
        
        elif "math" in prompt_lower or "calculate" in prompt_lower:
            return "I can help with math! For example: 2 + 2 = 4, and the square root of 16 is 4. This is a mock mathematical response for testing purposes."
        
        elif "story" in prompt_lower or "tale" in prompt_lower:
            return """Once upon a time, in a decentralized network far, far away, there lived a mock AI assistant. This assistant helped test the Crowd Models P2P network by providing consistent responses to various prompts. 

The assistant was happy to serve researchers and developers who were building the future of distributed AI systems. And they all lived efficiently ever after.

*This is a mock story response for testing.*"""
        
        else:
            return f"""This is a mock response generated by the test LLM server for the Crowd Models network.

Your prompt was: "{prompt[:100]}{'...' if len(prompt) > 100 else ''}"

Model: {model}
Time: {time.strftime('%Y-%m-%d %H:%M:%S')}

This response is generated for testing purposes and demonstrates that the P2P network is functioning correctly."""
    
    def log_message(self, format, *args):
        """Override to use our logger instead of stderr."""
        logger.info(format % args)

def main():
    parser = argparse.ArgumentParser(description='Mock LLM Server for Crowd Models testing')
    parser.add_argument('--port', type=int, default=8080, help='Port to listen on (default: 8080)')
    parser.add_argument('--host', type=str, default='localhost', help='Host to bind to (default: localhost)')
    
    args = parser.parse_args()
    
    server_address = (args.host, args.port)
    httpd = HTTPServer(server_address, MockLLMHandler)
    
    logger.info(f"Mock LLM Server starting on http://{args.host}:{args.port}")
    logger.info("Endpoint: POST /chat/completions")
    logger.info("Press Ctrl+C to stop the server")
    
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
        httpd.server_close()

if __name__ == '__main__':
    main()