#!/bin/bash
# load_test.sh - Load testing script for Order Matching Engine with trade scenario testing

# Configuration
BASE_URL="http://localhost:3000"
SYMBOLS=("Pranesh" "Superman" "Arnimzola" "Kathir")
DEFAULT_REQUESTS=100
DEFAULT_CONCURRENCY=10
DEFAULT_DELAY=0.01

# Default settings
num_requests=${1:-$DEFAULT_REQUESTS}
concurrency=${2:-$DEFAULT_CONCURRENCY}
delay=${3:-$DEFAULT_DELAY}

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to generate a random symbol
random_symbol() {
    # Choose one of the four symbols randomly
    local r=$((RANDOM % 4))
    if [ "$r" -eq 0 ]; then
        echo "Pranesh"
    elif [ "$r" -eq 1 ]; then
        echo "Superman"
    elif [ "$r" -eq 2 ]; then
        echo "Arnimzola"
    else
        echo "Kathir"
    fi
}

# Function to generate a random order as JSON
generate_order() {
    local symbol=$1
    local price=$2
    local qty=$3
    
    if [ -z "$symbol" ]; then
        symbol=$(random_symbol)
    fi
    
    if [ -z "$price" ]; then
        price=$((RANDOM % 990 + 10)) # 10-1000 price range
    fi
    
    if [ -z "$qty" ]; then
        qty=$((RANDOM % 99 + 1))     # 1-100 quantity range
    fi
    
    echo "{\"symbol\":\"$symbol\",\"price\":$price,\"qty\":$qty}"
}

# Function to send a buy order
send_buy_order() {
    local order=$1
    curl -s -X POST "$BASE_URL/buy" \
         -H "Content-Type: application/json" \
         -d "$order"
}

# Function to send a sell order
send_sell_order() {
    local order=$1
    curl -s -X POST "$BASE_URL/sell" \
         -H "Content-Type: application/json" \
         -d "$order"
}

# Function to extract order ID from response
extract_order_id() {
    local response=$1
    echo "$response" | grep -o '"order_id":[0-9]*' | cut -d':' -f2
}

# Function to process random orders
process_random_request() {
    local id=$1
    local side
    
    # Randomly choose buy or sell
    if [ $((RANDOM % 2)) -eq 0 ]; then
        side="buy"
    else
        side="sell"
    fi
    
    # Generate order
    local order=$(generate_order)
    
    # Send order
    if [ "$side" == "buy" ]; then
        response=$(send_buy_order "$order")
    else
        response=$(send_sell_order "$order")
    fi
    
    # Extract symbol from order
    symbol=$(echo $order | grep -o '"symbol":"[^"]*"' | cut -d'"' -f4)
    
    # Determine if successful
    if [[ $response == *"accepted"* ]]; then
        local order_id=$(extract_order_id "$response")
        echo -e "${GREEN}Request $id: $side order for $symbol (ID: $order_id) accepted${NC}"
    else
        echo -e "${RED}Request $id: $side order for $symbol FAILED: $response${NC}"
    fi
}

# Function to run trade scenario test (matching buy and sell orders)
run_trade_scenario() {
    local scenario_num=$1
    local symbol=$2
    local buy_price=$3
    local sell_price=$4
    local qty=$5
    
    echo -e "\n${YELLOW}=== Running Trade Scenario $scenario_num ===${NC}"
    echo -e "${BLUE}Symbol: $symbol, Buy price: $buy_price, Sell price: $sell_price, Qty: $qty${NC}"
    
    # Generate matching orders
    local buy_order=$(generate_order "$symbol" "$buy_price" "$qty")
    local sell_order=$(generate_order "$symbol" "$sell_price" "$qty")
    
    # Send buy order
    echo -e "${BLUE}Sending BUY order...${NC}"
    local buy_response=$(send_buy_order "$buy_order")
    local buy_order_id=$(extract_order_id "$buy_response")
    echo -e "${GREEN}BUY order sent, ID: $buy_order_id${NC}"
    echo "$buy_response" 
    
    # Small delay
    sleep 0.5
    
    # Send sell order
    echo -e "${BLUE}Sending SELL order...${NC}"
    local sell_response=$(send_sell_order "$sell_order")
    local sell_order_id=$(extract_order_id "$sell_response")
    echo -e "${GREEN}SELL order sent, ID: $sell_order_id${NC}"
    echo "$sell_response"
    
    # Give time for the trade to process
    echo -e "${YELLOW}Waiting for trade matching and egress processing...${NC}"
    sleep 1
    
    echo -e "${GREEN}Trade scenario $scenario_num completed. Check server logs for egress thread output.${NC}"
}

# Health check
echo -e "${BLUE}Performing health check...${NC}"
health_check=$(curl -s -X POST "$BASE_URL/health" -H "Content-Type: application/json" -d '{}')
if [ -z "$health_check" ]; then
    echo -e "${RED}Error: Server is not responding. Make sure it's running at $BASE_URL${NC}"
    exit 1
else
    echo -e "${GREEN}Health check passed!${NC}"
fi

# ===========================================================================
# PART 1: Test specific trade scenarios
# ===========================================================================
echo -e "\n${YELLOW}==================== TESTING TRADE SCENARIOS ====================${NC}"

# Scenario 1: Exact price match
run_trade_scenario 1 "Pranesh" 100 100 50

# Scenario 2: Buy price higher than sell price (should match)
run_trade_scenario 2 "Superman" 150 100 25

# Scenario 3: Multiple orders for same symbol
run_trade_scenario 3 "Arnimzola" 200 180 30
sleep 0.5
run_trade_scenario 4 "Arnimzola" 210 190 40

# ===========================================================================
# PART 2: Run general load test
# ===========================================================================
echo -e "\n${YELLOW}==================== RUNNING GENERAL LOAD TEST ====================${NC}"
echo -e "${BLUE}Starting load test with $num_requests requests at $concurrency concurrency...${NC}"

# Create temp directory for results
tmp_dir=$(mktemp -d)
results_file="$tmp_dir/results.txt"
touch "$results_file"  # Create the file

# Process requests with background jobs (limited concurrency)
start_time=$(date +%s)
active_jobs=0

# Process in batches to maintain concurrency
for i in $(seq 1 $num_requests); do
    # Limit concurrent jobs
    while [ $(jobs -p | wc -l) -ge $concurrency ]; do
        sleep 0.1
    done
    
    process_random_request $i &
    
    # Apply delay if needed
    if (( $(echo "$delay > 0" | bc 2>/dev/null || echo 0) )); then
        sleep $delay
    fi
    
    # Show progress every 10 requests
    if [ $((i % 10)) -eq 0 ]; then
        echo -e "${BLUE}Progress: $i/$num_requests requests${NC}"
    fi
done

# Wait for all jobs to finish
echo -e "${BLUE}Waiting for all requests to complete...${NC}"
wait
end_time=$(date +%s)

# Calculate total time
total_time=$((end_time - start_time))
requests_per_second=0
if [ $total_time -gt 0 ]; then
    requests_per_second=$((num_requests / total_time))
fi

# Print results
echo
echo -e "${YELLOW}=== Load Test Results ===${NC}"
echo "Timestamp: $(date '+%Y-%m-%dT%H:%M:%S')"
echo "Total requests: $num_requests"
echo "Elapsed time: $total_time seconds"
echo "Request rate: $requests_per_second requests/second"

# Clean up
rm -rf "$tmp_dir"

echo -e "\n${GREEN}Load test completed. Check server logs for egress thread output about trades.${NC}"
