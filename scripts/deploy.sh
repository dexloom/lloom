#!/bin/bash

# Deploy the Accounting smart contract to Sepolia testnet
# Requires foundry to be installed

set -e

echo "Deploying Accounting contract to Sepolia..."

# Check if PRIVATE_KEY is set
if [ -z "$PRIVATE_KEY" ]; then
    echo "Error: PRIVATE_KEY environment variable is not set"
    echo "Usage: PRIVATE_KEY=your_private_key_here ./scripts/deploy.sh"
    exit 1
fi

# Deploy the contract
forge create \
    --rpc-url https://rpc.sepolia.org \
    --private-key $PRIVATE_KEY \
    contracts/Accounting.sol:Accounting \
    --legacy

echo "Contract deployed successfully!"
echo "Don't forget to:"
echo "1. Save the contract address"
echo "2. Update your executor configuration"
echo "3. Fund your executor addresses with ETH for gas"