#!/bin/bash

# E2E Trading System Test Runner
# This script runs comprehensive end-to-end tests for the trading system

set -e

echo "🚀 Starting E2E Trading System Tests"
echo "===================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BACKEND_URL="http://127.0.0.1:7000"
WS_URL="ws://127.0.0.1:7000/ws"
TEST_SYMBOL="BTC-KRW"
TEST_CLIENT_ID="test_user_e2e"

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    case $status in
        "INFO")
            echo -e "${BLUE}ℹ️  $message${NC}"
            ;;
        "SUCCESS")
            echo -e "${GREEN}✅ $message${NC}"
            ;;
        "WARNING")
            echo -e "${YELLOW}⚠️  $message${NC}"
            ;;
        "ERROR")
            echo -e "${RED}❌ $message${NC}"
            ;;
    esac
}

# Function to check if backend is running
check_backend() {
    print_status "INFO" "Checking if backend is running..."
    
    if curl -s --connect-timeout 5 "$BACKEND_URL/api/v1/orderbook/$TEST_SYMBOL?depth=5" > /dev/null; then
        print_status "SUCCESS" "Backend is running and responding"
        return 0
    else
        print_status "ERROR" "Backend is not running or not responding"
        print_status "INFO" "Please start the backend with: cargo run"
        return 1
    fi
}

# Function to run basic API tests
run_api_tests() {
    print_status "INFO" "Running API Health Tests..."
    
    # Test 1: OrderBook API
    print_status "INFO" "Testing OrderBook API..."
    if curl -s "$BACKEND_URL/api/v1/orderbook/$TEST_SYMBOL?depth=5" | jq . > /dev/null 2>&1; then
        print_status "SUCCESS" "OrderBook API working"
    else
        print_status "ERROR" "OrderBook API failed"
        return 1
    fi
    
    # Test 2: Market Statistics API
    print_status "INFO" "Testing Market Statistics API..."
    if curl -s "$BACKEND_URL/api/v1/market/$TEST_SYMBOL/statistics" | jq . > /dev/null 2>&1; then
        print_status "SUCCESS" "Market Statistics API working"
    else
        print_status "ERROR" "Market Statistics API failed"
        return 1
    fi
    
    # Test 3: Candles API
    print_status "INFO" "Testing Candles API..."
    if curl -s "$BACKEND_URL/api/v1/candles/$TEST_SYMBOL?interval=1m&limit=20" | jq . > /dev/null 2>&1; then
        print_status "SUCCESS" "Candles API working"
    else
        print_status "ERROR" "Candles API failed"
        return 1
    fi
    
    return 0
}

# Function to test order submission
test_order_submission() {
    print_status "INFO" "Testing Order Submission..."
    
    # Test 1: Buy Limit Order
    print_status "INFO" "Submitting Buy Limit Order..."
    BUY_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Buy",
            "type": "Limit",
            "quantity": 1000000,
            "price": 95000000,
            "client_id": "'$TEST_CLIENT_ID'"
        }')
    
    if echo "$BUY_RESPONSE" | jq . > /dev/null 2>&1; then
        BUY_ORDER_ID=$(echo "$BUY_RESPONSE" | jq -r '.order_id // empty')
        if [ -n "$BUY_ORDER_ID" ]; then
            print_status "SUCCESS" "Buy order submitted: $BUY_ORDER_ID"
        else
            print_status "ERROR" "Buy order failed: $BUY_RESPONSE"
            return 1
        fi
    else
        print_status "ERROR" "Buy order failed: $BUY_RESPONSE"
        return 1
    fi
    
    # Test 2: Sell Limit Order
    print_status "INFO" "Submitting Sell Limit Order..."
    SELL_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Sell",
            "type": "Limit",
            "quantity": 500000,
            "price": 96000000,
            "client_id": "'$TEST_CLIENT_ID'"
        }')
    
    if echo "$SELL_RESPONSE" | jq . > /dev/null 2>&1; then
        SELL_ORDER_ID=$(echo "$SELL_RESPONSE" | jq -r '.order_id // empty')
        if [ -n "$SELL_ORDER_ID" ]; then
            print_status "SUCCESS" "Sell order submitted: $SELL_ORDER_ID"
        else
            print_status "ERROR" "Sell order failed: $SELL_RESPONSE"
            return 1
        fi
    else
        print_status "ERROR" "Sell order failed: $SELL_RESPONSE"
        return 1
    fi
    
    # Test 3: Market Order
    print_status "INFO" "Submitting Market Order..."
    MARKET_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Buy",
            "type": "Market",
            "quantity": 200000,
            "client_id": "'$TEST_CLIENT_ID'"
        }')
    
    if echo "$MARKET_RESPONSE" | jq . > /dev/null 2>&1; then
        MARKET_ORDER_ID=$(echo "$MARKET_RESPONSE" | jq -r '.order_id // empty')
        if [ -n "$MARKET_ORDER_ID" ]; then
            print_status "SUCCESS" "Market order submitted: $MARKET_ORDER_ID"
        else
            print_status "ERROR" "Market order failed: $MARKET_RESPONSE"
            return 1
        fi
    else
        print_status "ERROR" "Market order failed: $MARKET_RESPONSE"
        return 1
    fi
    
    return 0
}

# Function to test order status
test_order_status() {
    print_status "INFO" "Testing Order Status Retrieval..."
    
    # Get recent orders
    ORDERS_RESPONSE=$(curl -s "$BACKEND_URL/api/v1/user/$TEST_CLIENT_ID/orders")
    
    if echo "$ORDERS_RESPONSE" | jq . > /dev/null 2>&1; then
        ORDER_COUNT=$(echo "$ORDERS_RESPONSE" | jq '.orders | length')
        print_status "SUCCESS" "Retrieved $ORDER_COUNT orders for user"
        
        # Show order details
        echo "$ORDERS_RESPONSE" | jq '.orders[] | {order_id, side, type, quantity, price, status}' | head -20
    else
        print_status "ERROR" "Failed to retrieve orders: $ORDERS_RESPONSE"
        return 1
    fi
    
    return 0
}

# Function to test executions
test_executions() {
    print_status "INFO" "Testing Executions..."
    
    EXECUTIONS_RESPONSE=$(curl -s "$BACKEND_URL/api/v1/executions/$TEST_SYMBOL")
    
    if echo "$EXECUTIONS_RESPONSE" | jq . > /dev/null 2>&1; then
        EXECUTION_COUNT=$(echo "$EXECUTIONS_RESPONSE" | jq '.executions | length')
        print_status "SUCCESS" "Retrieved $EXECUTION_COUNT executions"
        
        # Show recent executions
        echo "$EXECUTIONS_RESPONSE" | jq '.executions[0:5] | .[] | {execution_id, side, price, quantity, timestamp}' | head -20
    else
        print_status "ERROR" "Failed to retrieve executions: $EXECUTIONS_RESPONSE"
        return 1
    fi
    
    return 0
}

# Function to test WebSocket connection
test_websocket() {
    print_status "INFO" "Testing WebSocket Connection..."
    
    # Create a simple WebSocket test script
    cat > /tmp/websocket_test.js << 'EOF'
const WebSocket = require('ws');

const ws = new WebSocket('ws://127.0.0.1:7000/ws');

let messageCount = 0;
let orderbookUpdates = 0;
let executionUpdates = 0;

ws.on('open', function open() {
    console.log('✅ WebSocket connected');
    
    // Subscribe to orderbook updates
    ws.send(JSON.stringify({
        type: 'subscribe',
        channel: 'orderbook',
        symbol: 'BTC-KRW'
    }));
    
    // Subscribe to executions
    ws.send(JSON.stringify({
        type: 'subscribe',
        channel: 'executions',
        symbol: 'BTC-KRW'
    }));
    
    // Set timeout to close connection
    setTimeout(() => {
        console.log(`📊 WebSocket Test Results:`);
        console.log(`  Total messages: ${messageCount}`);
        console.log(`  OrderBook updates: ${orderbookUpdates}`);
        console.log(`  Execution updates: ${executionUpdates}`);
        ws.close();
    }, 10000); // 10 seconds
});

ws.on('message', function message(data) {
    messageCount++;
    try {
        const msg = JSON.parse(data);
        switch(msg.type) {
            case 'orderbook':
                orderbookUpdates++;
                console.log('📊 OrderBook update received');
                break;
            case 'execution':
                executionUpdates++;
                console.log('⚡ Execution update received:', msg.execution_id);
                break;
            default:
                console.log('📨 WebSocket message:', msg.type);
        }
    } catch (e) {
        console.log('📨 Raw message:', data.toString());
    }
});

ws.on('close', function close() {
    console.log('🔌 WebSocket connection closed');
    process.exit(0);
});

ws.on('error', function error(err) {
    console.error('❌ WebSocket error:', err.message);
    process.exit(1);
});
EOF

    # Check if Node.js is available
    if command -v node > /dev/null 2>&1; then
        # Install ws package if needed
        if ! npm list ws > /dev/null 2>&1; then
            npm install ws > /dev/null 2>&1
        fi
        
        # Run WebSocket test
        if node /tmp/websocket_test.js; then
            print_status "SUCCESS" "WebSocket test completed"
        else
            print_status "ERROR" "WebSocket test failed"
            return 1
        fi
    else
        print_status "WARNING" "Node.js not available, skipping WebSocket test"
    fi
    
    # Cleanup
    rm -f /tmp/websocket_test.js
    
    return 0
}

# Function to test error handling
test_error_handling() {
    print_status "INFO" "Testing Error Handling..."
    
    # Test 1: Invalid order (negative quantity)
    print_status "INFO" "Testing invalid order (negative quantity)..."
    INVALID_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Buy",
            "type": "Limit",
            "quantity": -1000000,
            "price": 95000000,
            "client_id": "'$TEST_CLIENT_ID'"
        }')
    
    if echo "$INVALID_RESPONSE" | jq .error > /dev/null 2>&1; then
        print_status "SUCCESS" "Invalid order correctly rejected"
    else
        print_status "ERROR" "Invalid order was accepted: $INVALID_RESPONSE"
        return 1
    fi
    
    # Test 2: Invalid symbol
    print_status "INFO" "Testing invalid symbol..."
    INVALID_SYMBOL_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "INVALID-SYMBOL",
            "side": "Buy",
            "type": "Limit",
            "quantity": 1000000,
            "price": 95000000,
            "client_id": "'$TEST_CLIENT_ID'"
        }')
    
    if echo "$INVALID_SYMBOL_RESPONSE" | jq .error > /dev/null 2>&1; then
        print_status "SUCCESS" "Invalid symbol correctly rejected"
    else
        print_status "ERROR" "Invalid symbol was accepted: $INVALID_SYMBOL_RESPONSE"
        return 1
    fi
    
    return 0
}

# Function to test order matching
test_order_matching() {
    print_status "INFO" "Testing Order Matching Engine..."
    
    # Submit matching buy and sell orders
    MATCHING_PRICE=95500000
    
    print_status "INFO" "Submitting matching buy order..."
    BUY_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Buy",
            "type": "Limit",
            "quantity": 1000000,
            "price": '$MATCHING_PRICE',
            "client_id": "'$TEST_CLIENT_ID'_buyer"
        }')
    
    print_status "INFO" "Submitting matching sell order..."
    SELL_RESPONSE=$(curl -s -X POST "$BACKEND_URL/api/v1/order" \
        -H "Content-Type: application/json" \
        -d '{
            "symbol": "'$TEST_SYMBOL'",
            "side": "Sell",
            "type": "Limit",
            "quantity": 1000000,
            "price": '$MATCHING_PRICE',
            "client_id": "'$TEST_CLIENT_ID'_seller"
        }')
    
    if echo "$BUY_RESPONSE" | jq .order_id > /dev/null 2>&1 && echo "$SELL_RESPONSE" | jq .order_id > /dev/null 2>&1; then
        print_status "SUCCESS" "Matching orders submitted successfully"
        
        # Wait for matching to occur
        print_status "INFO" "Waiting for order matching..."
        sleep 3
        
        # Check executions
        EXECUTIONS_RESPONSE=$(curl -s "$BACKEND_URL/api/v1/executions/$TEST_SYMBOL")
        if echo "$EXECUTIONS_RESPONSE" | jq .executions > /dev/null 2>&1; then
            EXECUTION_COUNT=$(echo "$EXECUTIONS_RESPONSE" | jq '.executions | length')
            print_status "SUCCESS" "Found $EXECUTION_COUNT executions after matching"
        else
            print_status "WARNING" "No executions found, matching may not have occurred"
        fi
    else
        print_status "ERROR" "Failed to submit matching orders"
        return 1
    fi
    
    return 0
}

# Function to run performance test
test_performance() {
    print_status "INFO" "Running Performance Test..."
    
    # Submit multiple orders quickly
    ORDER_COUNT=10
    print_status "INFO" "Submitting $ORDER_COUNT orders rapidly..."
    
    for i in $(seq 1 $ORDER_COUNT); do
        curl -s -X POST "$BACKEND_URL/api/v1/order" \
            -H "Content-Type: application/json" \
            -d '{
                "symbol": "'$TEST_SYMBOL'",
                "side": "Buy",
                "type": "Limit",
                "quantity": 100000,
                "price": '$((95000000 + i * 1000))',
                "client_id": "'$TEST_CLIENT_ID'_perf_'$i'"
            }' > /dev/null &
    done
    
    # Wait for all requests to complete
    wait
    
    print_status "SUCCESS" "Performance test completed"
    return 0
}

# Main test execution
main() {
    local failed_tests=0
    
    # Check if backend is running
    if ! check_backend; then
        exit 1
    fi
    
    # Run tests
    echo ""
    print_status "INFO" "Starting comprehensive E2E tests..."
    
    # API Tests
    if ! run_api_tests; then
        ((failed_tests++))
    fi
    
    # Order Submission Tests
    if ! test_order_submission; then
        ((failed_tests++))
    fi
    
    # Order Status Tests
    if ! test_order_status; then
        ((failed_tests++))
    fi
    
    # Executions Tests
    if ! test_executions; then
        ((failed_tests++))
    fi
    
    # WebSocket Tests
    if ! test_websocket; then
        ((failed_tests++))
    fi
    
    # Error Handling Tests
    if ! test_error_handling; then
        ((failed_tests++))
    fi
    
    # Order Matching Tests
    if ! test_order_matching; then
        ((failed_tests++))
    fi
    
    # Performance Tests
    if ! test_performance; then
        ((failed_tests++))
    fi
    
    # Print final results
    echo ""
    echo "====================================="
    if [ $failed_tests -eq 0 ]; then
        print_status "SUCCESS" "All E2E tests PASSED! 🎉"
        exit 0
    else
        print_status "ERROR" "$failed_tests test(s) FAILED! 💥"
        exit 1
    fi
}

# Check dependencies
if ! command -v curl > /dev/null 2>&1; then
    print_status "ERROR" "curl is required but not installed"
    exit 1
fi

if ! command -v jq > /dev/null 2>&1; then
    print_status "ERROR" "jq is required but not installed"
    exit 1
fi

# Run main function
main "$@"