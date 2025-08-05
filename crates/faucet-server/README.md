# Lloom Faucet Server

An Ethereum faucet server that provides ETH to addresses via email verification. This server is integrated with the Lloom ecosystem and follows the existing patterns and infrastructure.

## Features

- **Email Verification**: Users provide email and Ethereum address, receive verification token via email
- **Configurable Funding**: Set target ETH amount (default: 1 ETH)
- **Rate Limiting**: Configurable limits per email and IP address
- **Security**: Token expiration, input validation, and comprehensive error handling
- **Monitoring**: Health checks and detailed logging
- **Integration**: Uses existing Lloom blockchain infrastructure and patterns

## Quick Start

1. **Generate Configuration**:
   ```bash
   cargo run --bin faucet-server -- --generate-config
   ```

2. **Edit Configuration**:
   Edit the generated `faucet-config.toml` file:
   - Set your Ethereum private key (wallet that will send ETH)
   - Configure SMTP settings for email sending
   - Adjust security and rate limiting settings

3. **Run the Server**:
   ```bash
   cargo run --bin faucet-server -- --config faucet-config.toml
   ```

## API Endpoints

### `POST /request`
Request a faucet token by providing email and Ethereum address.

**Request Body**:
```json
{
  "email": "user@example.com",
  "ethereum_address": "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a"
}
```

**Response**:
```json
{
  "message": "Verification token sent to user@example.com. Please check your email and use the token to redeem funds."
}
```

### `POST /redeem`
Redeem a token programmatically to receive ETH funding (JSON API).

**Request Body**:
```json
{
  "token": "your-verification-token-from-email"
}
```

**Response**:
```json
{
  "message": "Successfully funded address 0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a with ETH. Transaction: 0xabc123...",
  "transaction_hash": "0xabc123...",
  "ethereum_address": "0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a"
}
```

### `GET /redeem/{token}`
Redeem a token through a browser interface to receive ETH funding (HTML response).

This endpoint provides an HTML response for web-based token redemption, making it easy for users to redeem tokens by clicking on links or visiting URLs directly in their browser.

**URL Format**:
```
GET /redeem/{token}
```

**Parameters**:
- `token` (path parameter): The verification token from email

**Response**:
- Returns HTML content with success or error messages
- `200 OK`: HTML page confirming successful token redemption with transaction details
- `400 Bad Request`: HTML page with error message for invalid token format
- `404 Not Found`: HTML page indicating token not found or already used
- `500 Internal Server Error`: HTML page with server error message

**Example**:
```
http://localhost:3030/redeem/your-verification-token-from-email
```

Users can visit this URL in their browser to redeem tokens and see a user-friendly HTML page showing the redemption result.

### `GET /health`
Health check endpoint.

**Response**:
```json
{
  "status": "healthy",
  "ethereum_connected": true,
  "email_configured": true,
  "active_tokens": 5
}
```

### `GET /`
Server information and endpoint documentation.

## Configuration

The server uses a TOML configuration file with the following sections:

### HTTP Configuration
```toml
[http]
port = 3030
bind_address = "127.0.0.1"
```

### Ethereum Configuration
```toml
[ethereum]
rpc_url = "https://rpc.sepolia.org"
private_key = "your_64_character_hex_private_key"
target_amount_eth = 1.0
gas_multiplier = 1.2
min_faucet_balance_eth = 10.0
```

### SMTP Configuration
```toml
[smtp]
server = "smtp.gmail.com"
port = 587
username = "your_email@gmail.com"
password = "your_app_password"
from_address = "your_email@gmail.com"
subject = "Your Faucet Token"
```

### Security Configuration
```toml
[security]
token_expiry_minutes = 15
max_requests_per_email_per_day = 1
max_requests_per_ip_per_hour = 5
cleanup_interval_minutes = 30
```

## Environment Variables

You can override configuration values using environment variables with the `FAUCET_` prefix:

```bash
export FAUCET_ETHEREUM_PRIVATE_KEY="your_private_key"
export FAUCET_SMTP_PASSWORD="your_smtp_password"
cargo run --bin faucet-server
```

## Security Considerations

1. **Private Key**: Store your Ethereum private key securely. Consider using environment variables or secure key management systems.

2. **SMTP Credentials**: Use app passwords for Gmail or secure SMTP credentials.

3. **Rate Limiting**: Configure appropriate rate limits to prevent abuse.

4. **Network**: Consider running behind a reverse proxy with additional security measures.

5. **Monitoring**: Monitor the faucet wallet balance and transaction activity.

## Example Usage

1. **Request Token**:
   ```bash
   curl -X POST http://localhost:3030/request \
     -H "Content-Type: application/json" \
     -d '{"email":"user@example.com","ethereum_address":"0x742d35Cc6634C0532925a3b8D404cB8b3d3A5d3a"}'
   ```

2. **Check Email** and get the verification token

3. **Redeem Token** (choose one method):

   **Method A: Programmatic redemption (JSON API)**:
   ```bash
   curl -X POST http://localhost:3030/redeem \
     -H "Content-Type: application/json" \
     -d '{"token":"your-token-from-email"}'
   ```

   **Method B: Browser-based redemption (HTML interface)**:
   
   Simply visit the redemption URL in your browser:
   ```
   http://localhost:3030/redeem/your-token-from-email
   ```
   
   This will display a user-friendly HTML page showing whether the token redemption was successful or if there were any errors.

## Development

### Running Tests
```bash
cargo test -p faucet-server
```

### Building
```bash
cargo build --bin faucet-server --release
```

### Docker Support
You can create a Dockerfile for containerized deployment:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin faucet-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/faucet-server /usr/local/bin/
EXPOSE 3030
CMD ["faucet-server"]
```

## Troubleshooting

### Common Issues

1. **Invalid Private Key**: Ensure your private key is 64 hex characters without the `0x` prefix.

2. **SMTP Authentication**: For Gmail, use an app password instead of your regular password.

3. **Insufficient Balance**: Ensure your faucet wallet has enough ETH to fund requests.

4. **Rate Limiting**: If requests are being rejected, check the rate limiting configuration.

### Logs

The server provides detailed logging. Set the log level using the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run --bin faucet-server
```

## Integration with Lloom

This faucet server is designed to integrate seamlessly with the existing Lloom ecosystem:

- Uses the same `alloy` Ethereum client infrastructure
- Follows existing configuration patterns with TOML files
- Uses the same logging and error handling approaches
- Maintains consistency with other Lloom crates

## License

This project is licensed under the MIT License - see the main repository license for details.
