#!/bin/bash

# transfer_test_funds.sh - Transfer funds from RETH dev mode test addresses
# 
# This script transfers funds from the 20 pre-funded test addresses to a target address.
# The test addresses are derived from the standard test mnemonic used by RETH dev mode.

set -euo pipefail

# Configuration
MNEMONIC="test test test test test test test test test test test junk"
RPC_URL="http://localhost:8545"
CHAIN_ID=1337

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
AMOUNT=""
TARGET_ADDRESS=""
START_INDEX=0
END_INDEX=19
DRY_RUN=false

# Usage function
usage() {
    echo "Usage: $0 --target <address> --amount <amount> [options]"
    echo ""
    echo "Required:"
    echo "  --target <address>    Target address to receive funds"
    echo "  --amount <amount>     Amount to transfer from each address (in ETH)"
    echo ""
    echo "Options:"
    echo "  --start <index>       Start index (0-19, default: 0)"
    echo "  --end <index>         End index (0-19, default: 19)"
    echo "  --dry-run            Show what would be transferred without executing"
    echo "  --rpc-url <url>      RPC URL (default: http://localhost:8545)"
    echo "  --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 1.5"
    echo "  $0 --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 0.1 --start 0 --end 4"
    echo "  $0 --target 0x742d35Cc6634C0532925a3b8D6f5DDA5CF21F0a7 --amount 2.0 --dry-run"
}

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if required tools are available
check_dependencies() {
    if ! command -v cast &> /dev/null; then
        log_error "cast command not found. Please install Foundry: https://getfoundry.sh"
        exit 1
    fi
    
    # Check if bc is available for numerical comparisons (optional but recommended)
    if ! command -v bc &> /dev/null; then
        log_warning "bc command not found. Using bash arithmetic instead (less precise for large numbers)."
    fi
}

# Validate Ethereum address
validate_address() {
    local address=$1
    if [[ ! $address =~ ^0x[a-fA-F0-9]{40}$ ]]; then
        log_error "Invalid Ethereum address: $address"
        exit 1
    fi
}

# Validate amount
validate_amount() {
    local amount=$1
    if ! [[ $amount =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
        log_error "Invalid amount: $amount (must be a positive number)"
        exit 1
    fi
}

# Derive address from mnemonic and index
derive_address() {
    local index=$1
    local derivation_path="m/44'/60'/0'/0/$index"
    
    # Use cast to derive the address
    cast wallet address --mnemonic "$MNEMONIC" --mnemonic-derivation-path "$derivation_path" 2>/dev/null
}

# Get private key from mnemonic and index
derive_private_key() {
    local index=$1
    local derivation_path="m/44'/60'/0'/0/$index"
    
    # Use cast to derive the private key
    cast wallet private-key --mnemonic "$MNEMONIC" --mnemonic-derivation-path "$derivation_path" 2>/dev/null
}

# Get balance of an address
get_balance() {
    local address=$1
    cast balance "$address" --rpc-url "$RPC_URL" 2>/dev/null || echo "0"
}

# Format balance from wei to ETH
format_balance() {
    local balance_wei=$1
    cast to-unit "$balance_wei" ether 2>/dev/null || echo "0"
}

# Convert ETH to wei
eth_to_wei() {
    local eth_amount=$1
    cast to-wei "$eth_amount" ether 2>/dev/null
}

# Check if RPC is available
check_rpc_connection() {
    log_info "Checking RPC connection to $RPC_URL..."
    if ! cast chain-id --rpc-url "$RPC_URL" &>/dev/null; then
        log_error "Cannot connect to RPC at $RPC_URL"
        log_error "Make sure your RETH node is running in dev mode"
        exit 1
    fi
    log_success "Connected to RPC at $RPC_URL"
}

# Show balances of all test addresses
show_balances() {
    log_info "Current balances of test addresses:"
    echo "----------------------------------------"
    printf "%-4s %-42s %-20s\n" "Idx" "Address" "Balance (ETH)"
    echo "----------------------------------------"
    
    for i in $(seq $START_INDEX $END_INDEX); do
        local address
        address=$(derive_address $i)
        local balance_wei
        balance_wei=$(get_balance "$address")
        local balance_eth
        balance_eth=$(format_balance "$balance_wei")
        printf "%-4s %-42s %-20s\n" "$i" "$address" "$balance_eth"
    done
    echo "----------------------------------------"
}

# Transfer funds from a single address
transfer_from_address() {
    local index=$1
    local from_address=$2
    local private_key=$3
    local amount_wei=$4
    
    log_info "Transferring from address $index ($from_address)..."
    
    # Check if address has enough balance
    local current_balance
    current_balance=$(get_balance "$from_address")
    
    # Compare balances (use bc if available, otherwise bash arithmetic)
    local insufficient_balance=false
    if command -v bc &> /dev/null; then
        if [[ $(echo "$current_balance < $amount_wei" | bc) == "1" ]]; then
            insufficient_balance=true
        fi
    else
        # Fallback to bash arithmetic (less precise for very large numbers)
        if [[ $current_balance -lt $amount_wei ]] 2>/dev/null; then
            insufficient_balance=true
        fi
    fi
    
    if [[ "$insufficient_balance" == "true" ]]; then
        local balance_eth
        balance_eth=$(format_balance "$current_balance")
        local amount_eth
        amount_eth=$(format_balance "$amount_wei")
        log_warning "Address $index has insufficient balance: $balance_eth ETH < $amount_eth ETH"
        return 1
    fi
    
    if [[ "$DRY_RUN" == "true" ]]; then
        local amount_eth
        amount_eth=$(format_balance "$amount_wei")
        log_info "[DRY RUN] Would transfer $amount_eth ETH from $from_address to $TARGET_ADDRESS"
        return 0
    fi
    
    # Execute the transfer
    local tx_hash
    if tx_hash=$(cast send "$TARGET_ADDRESS" \
        --value "$amount_wei" \
        --private-key "$private_key" \
        --rpc-url "$RPC_URL" \
        --chain "$CHAIN_ID" 2>/dev/null); then
        log_success "Transfer successful! TX: $tx_hash"
        return 0
    else
        log_error "Transfer failed from address $index"
        return 1
    fi
}

# Main transfer function
execute_transfers() {
    local amount_wei
    amount_wei=$(eth_to_wei "$AMOUNT")
    
    log_info "Starting transfers..."
    log_info "Target address: $TARGET_ADDRESS"
    log_info "Amount per transfer: $AMOUNT ETH ($amount_wei wei)"
    log_info "Address range: $START_INDEX to $END_INDEX"
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_warning "DRY RUN MODE - No actual transfers will be executed"
    fi
    
    echo ""
    
    local successful_transfers=0
    local failed_transfers=0
    
    for i in $(seq $START_INDEX $END_INDEX); do
        local address
        address=$(derive_address $i)
        local private_key
        private_key=$(derive_private_key $i)
        
        if transfer_from_address "$i" "$address" "$private_key" "$amount_wei"; then
            ((successful_transfers++))
        else
            ((failed_transfers++))
        fi
        
        # Small delay between transfers
        sleep 0.1
    done
    
    echo ""
    log_info "Transfer summary:"
    log_success "Successful transfers: $successful_transfers"
    if [[ $failed_transfers -gt 0 ]]; then
        log_warning "Failed transfers: $failed_transfers"
    fi
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --target)
                TARGET_ADDRESS="$2"
                shift 2
                ;;
            --amount)
                AMOUNT="$2"
                shift 2
                ;;
            --start)
                START_INDEX="$2"
                shift 2
                ;;
            --end)
                END_INDEX="$2"
                shift 2
                ;;
            --rpc-url)
                RPC_URL="$2"
                shift 2
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
}

# Validate arguments
validate_args() {
    if [[ -z "$TARGET_ADDRESS" ]]; then
        log_error "Target address is required"
        usage
        exit 1
    fi
    
    if [[ -z "$AMOUNT" ]]; then
        log_error "Amount is required"
        usage
        exit 1
    fi
    
    validate_address "$TARGET_ADDRESS"
    validate_amount "$AMOUNT"
    
    if [[ $START_INDEX -lt 0 || $START_INDEX -gt 19 ]]; then
        log_error "Start index must be between 0 and 19"
        exit 1
    fi
    
    if [[ $END_INDEX -lt 0 || $END_INDEX -gt 19 ]]; then
        log_error "End index must be between 0 and 19"
        exit 1
    fi
    
    if [[ $START_INDEX -gt $END_INDEX ]]; then
        log_error "Start index cannot be greater than end index"
        exit 1
    fi
}

# Main function
main() {
    echo "🚀 RETH Test Fund Transfer Script"
    echo "================================="
    echo ""
    
    parse_args "$@"
    validate_args
    check_dependencies
    check_rpc_connection
    
    echo ""
    show_balances
    echo ""
    
    if [[ "$DRY_RUN" == "false" ]]; then
        read -p "Proceed with transfers? [y/N] " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Transfer cancelled by user"
            exit 0
        fi
        echo ""
    fi
    
    execute_transfers
    
    if [[ "$DRY_RUN" == "false" ]]; then
        echo ""
        log_info "Updated balances:"
        show_balances
    fi
    
    echo ""
    log_success "Script completed successfully!"
}

# Run main function with all arguments
main "$@"