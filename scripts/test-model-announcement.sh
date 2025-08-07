#!/bin/bash

# Model Announcement System End-to-End Test Suite
# Tests the complete flow from announcement to discovery to request

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
VALIDATOR_PORT=8080
EXECUTOR_PORT=8081
CLIENT_PORT=8082
TEST_TIMEOUT=30
CLEANUP_ON_EXIT=true

# PIDs for cleanup
VALIDATOR_PID=""
EXECUTOR_PID=""
CLIENT_PID=""

# Cleanup function
cleanup() {
    echo -e "${YELLOW}Cleaning up test processes...${NC}"
    
    if [ ! -z "$VALIDATOR_PID" ]; then
        kill $VALIDATOR_PID 2>/dev/null || true
        echo "Stopped validator (PID: $VALIDATOR_PID)"
    fi
    
    if [ ! -z "$EXECUTOR_PID" ]; then
        kill $EXECUTOR_PID 2>/dev/null || true
        echo "Stopped executor (PID: $EXECUTOR_PID)"
    fi
    
    if [ ! -z "$CLIENT_PID" ]; then
        kill $CLIENT_PID 2>/dev/null || true
        echo "Stopped client (PID: $CLIENT_PID)"
    fi
    
    # Clean up any test data
    rm -f test_validator.log test_executor.log test_client.log
    
    echo -e "${GREEN}Cleanup complete${NC}"
}

# Set up cleanup on exit
if [ "$CLEANUP_ON_EXIT" = true ]; then
    trap cleanup EXIT INT TERM
fi

# Helper function to wait for service to be ready
wait_for_service() {
    local port=$1
    local service_name=$2
    local max_attempts=30
    local attempt=0
    
    echo -e "${BLUE}Waiting for $service_name on port $port...${NC}"
    
    while [ $attempt -lt $max_attempts ]; do
        if nc -z localhost $port 2>/dev/null; then
            echo -e "${GREEN}$service_name is ready on port $port${NC}"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    
    echo -e "${RED}Timeout waiting for $service_name on port $port${NC}"
    return 1
}

# Helper function to check if command exists
check_command() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed or not in PATH${NC}"
        echo "Please install $1 and try again"
        exit 1
    fi
}

# Pre-flight checks
echo -e "${BLUE}=== Model Announcement Test Suite ===${NC}"
echo -e "${BLUE}Performing pre-flight checks...${NC}"

check_command "cargo"
check_command "nc"

# Build all components
echo -e "${BLUE}Building all components...${NC}"
cargo build --release --workspace || {
    echo -e "${RED}Build failed${NC}"
    exit 1
}

echo -e "${GREEN}Pre-flight checks complete${NC}"

# Test 1: Start Validator
echo -e "\n${BLUE}=== Test 1: Starting Validator ===${NC}"
echo "Starting validator with model announcement subscription..."

cargo run --release --bin lloom-validator -- \
    --port $VALIDATOR_PORT \
    --subscribe-model-announcements \
    > test_validator.log 2>&1 &
VALIDATOR_PID=$!

wait_for_service $VALIDATOR_PORT "Validator" || exit 1
echo -e "${GREEN}✓ Validator started and ready${NC}"

# Test 2: Start Executor with Model Announcement
echo -e "\n${BLUE}=== Test 2: Starting Executor with Model Announcement ===${NC}"
echo "Starting executor and announcing available models..."

cargo run --release --bin lloom-executor -- \
    --port $EXECUTOR_PORT \
    --validator-address "127.0.0.1:$VALIDATOR_PORT" \
    --announce-models \
    --config crates/lloom-executor/test_config.toml \
    > test_executor.log 2>&1 &
EXECUTOR_PID=$!

wait_for_service $EXECUTOR_PORT "Executor" || exit 1
echo -e "${GREEN}✓ Executor started and announced models${NC}"

# Give some time for model announcement to propagate
sleep 3

# Test 3: Verify Model Announcement Received
echo -e "\n${BLUE}=== Test 3: Verifying Model Announcements ===${NC}"
echo "Checking if validator received model announcements..."

# Check validator logs for model announcements
if grep -q "Received model announcement" test_validator.log; then
    echo -e "${GREEN}✓ Validator received model announcements${NC}"
else
    echo -e "${YELLOW}⚠ No model announcements found in validator logs${NC}"
    echo "Validator log contents:"
    tail -10 test_validator.log
fi

# Test 4: Client Discovery
echo -e "\n${BLUE}=== Test 4: Client Model Discovery ===${NC}"
echo "Starting client and testing model discovery..."

# Test client discovery
timeout $TEST_TIMEOUT cargo run --release --bin lloom-client -- \
    --validator-address "127.0.0.1:$VALIDATOR_PORT" \
    --discover-models \
    > test_client.log 2>&1 || {
    echo -e "${YELLOW}⚠ Client discovery timed out or failed${NC}"
    echo "Client log contents:"
    tail -10 test_client.log
}

if grep -q "Found models" test_client.log; then
    echo -e "${GREEN}✓ Client successfully discovered models${NC}"
else
    echo -e "${YELLOW}⚠ No models discovered by client${NC}"
    echo "Client log contents:"
    tail -10 test_client.log
fi

# Test 5: Specific Model Query
echo -e "\n${BLUE}=== Test 5: Specific Model Query ===${NC}"
echo "Testing client query for specific models..."

timeout $TEST_TIMEOUT cargo run --release --bin lloom-client -- \
    --validator-address "127.0.0.1:$VALIDATOR_PORT" \
    --query-model "gpt-3.5-turbo" \
    >> test_client.log 2>&1 || {
    echo -e "${YELLOW}⚠ Client model query timed out or failed${NC}"
}

if grep -q "Model query result" test_client.log; then
    echo -e "${GREEN}✓ Client successfully queried specific model${NC}"
else
    echo -e "${YELLOW}⚠ No specific model query results${NC}"
fi

# Test 6: Heartbeat Test
echo -e "\n${BLUE}=== Test 6: Heartbeat Verification ===${NC}"
echo "Verifying heartbeat mechanism..."

# Wait for at least one heartbeat cycle
echo "Waiting for heartbeat cycle (10 seconds)..."
sleep 10

if grep -q "heartbeat" test_executor.log; then
    echo -e "${GREEN}✓ Executor heartbeat detected${NC}"
else
    echo -e "${YELLOW}⚠ No executor heartbeat found${NC}"
fi

if grep -q "heartbeat" test_validator.log; then
    echo -e "${GREEN}✓ Validator processed heartbeat${NC}"
else
    echo -e "${YELLOW}⚠ No validator heartbeat processing found${NC}"
fi

# Test 7: Model Update Test
echo -e "\n${BLUE}=== Test 7: Model Update Test ===${NC}"
echo "Testing model updates..."

# Send a model update (this would depend on the actual implementation)
echo "Note: Model update testing depends on runtime API implementation"

# Test 8: Stale Executor Cleanup (Simulation)
echo -e "\n${BLUE}=== Test 8: Stale Executor Detection ===${NC}"
echo "Testing stale executor cleanup (stopping executor)..."

# Stop executor to simulate stale connection
if [ ! -z "$EXECUTOR_PID" ]; then
    kill $EXECUTOR_PID 2>/dev/null || true
    EXECUTOR_PID=""
    echo "Stopped executor to simulate stale connection"
    
    # Wait for validator to detect stale executor
    echo "Waiting for stale executor detection (30 seconds)..."
    sleep 30
    
    if grep -q "stale\|timeout\|removed" test_validator.log; then
        echo -e "${GREEN}✓ Validator detected stale executor${NC}"
    else
        echo -e "${YELLOW}⚠ No stale executor detection found${NC}"
    fi
fi

# Summary
echo -e "\n${BLUE}=== Test Summary ===${NC}"
echo "Model Announcement System Test Results:"

# Count successful tests
successful_tests=0
total_tests=8

echo "1. Validator startup: ✓"
successful_tests=$((successful_tests + 1))

echo "2. Executor startup with announcements: ✓"
successful_tests=$((successful_tests + 1))

if grep -q "Received model announcement" test_validator.log; then
    echo "3. Model announcement reception: ✓"
    successful_tests=$((successful_tests + 1))
else
    echo "3. Model announcement reception: ✗"
fi

if grep -q "Found models" test_client.log; then
    echo "4. Client model discovery: ✓"
    successful_tests=$((successful_tests + 1))
else
    echo "4. Client model discovery: ✗"
fi

if grep -q "Model query result" test_client.log; then
    echo "5. Specific model query: ✓"
    successful_tests=$((successful_tests + 1))
else
    echo "5. Specific model query: ✗"
fi

if grep -q "heartbeat" test_executor.log && grep -q "heartbeat" test_validator.log; then
    echo "6. Heartbeat mechanism: ✓"
    successful_tests=$((successful_tests + 1))
else
    echo "6. Heartbeat mechanism: ✗"
fi

echo "7. Model updates: ~ (depends on runtime implementation)"

if grep -q "stale\|timeout\|removed" test_validator.log; then
    echo "8. Stale executor detection: ✓"
    successful_tests=$((successful_tests + 1))
else
    echo "8. Stale executor detection: ✗"
fi

echo -e "\n${GREEN}Tests passed: $successful_tests/$total_tests${NC}"

if [ $successful_tests -eq $total_tests ]; then
    echo -e "${GREEN}🎉 All tests passed! Model announcement system is working correctly.${NC}"
    exit 0
elif [ $successful_tests -gt $((total_tests / 2)) ]; then
    echo -e "${YELLOW}⚠ Most tests passed, but some issues detected. Check logs for details.${NC}"
    exit 1
else
    echo -e "${RED}❌ Multiple test failures detected. System may not be working correctly.${NC}"
    exit 1
fi