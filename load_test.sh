#!/bin/bash
# load_test.sh - Load testing script for Order Matching Engine

# Configuration
BASE_URL="http://localhost:3000"
SYMBOLS=("Pranesh" "Superman" "Arnimzola")
DEFAULT_REQUESTS=1000
DEFAULT_CONCURRENCY=50
DEFAULT_DELAY=0.001

# Default settings
num_requests=${1:-$DEFAULT_REQUESTS}
concurrency=${2:-$DEFAULT_CONCURRENCY}
delay=${3:-$DEFAULT_DELAY}

# Function to generate a random symbol
random_symbol() {
    # Choose one of the three symbols randomly
    local r=$((RANDOM % 3))
    if [ "$r" -eq 0 ]; then
        echo "Pranesh"
    elif [ "$r" -eq 1 ]; then
        echo "Superman"
    else
        echo "Arnimzola"
    fi
}

# Function to generate a random order as JSON
generate_order() {
    local symbol=$(random_symbol)
    local price=$((RANDOM % 990 + 10)) # 10-1000 price range
    local qty=$((RANDOM % 99 + 1))     # 1-100 quantity range
    
    echo "{\"symbol\":\"$symbol\",\"price\":$price,\"qty\":$qty}"
}

# Function to send a buy order
send_buy_order() {
    local order=$1
    curl -s -X POST "$BASE_URL/buy" \
         -H "Content-Type: application/json" \
         -d "$order"
    echo
}

# Function to send a sell order
send_sell_order() {
    local order=$1
    curl -s -X POST "$BASE_URL/sell" \
         -H "Content-Type: application/json" \
         -d "$order"
    echo
}

# Health check
echo "Performing health check..."
health_check=$(curl -s -X POST "$BASE_URL/health" -H "Content-Type: application/json" -d '{}')
if [ -z "$health_check" ]; then
    echo "Error: Server is not responding. Make sure it's running at $BASE_URL"
    exit 1
else
    echo "Health check passed! Starting load test..."
fi

# Print test parameters
echo "Starting load test with $num_requests requests at $concurrency concurrency..."

# Create temp directory for results
tmp_dir=$(mktemp -d)
results_file="$tmp_dir/results.txt"
touch "$results_file"  # Create the file

# Function to process one request
process_request() {
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
    local start_time=$(date +%s)
    if [ "$side" == "buy" ]; then
        response=$(send_buy_order "$order")
    else
        response=$(send_sell_order "$order")
    fi
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # Extract order info from JSON for result reporting
    symbol=$(echo $order | grep -o '"symbol":"[^"]*"' | cut -d'"' -f4)
    
    # Determine if successful
    if [[ $response == *"accepted"* ]]; then
        echo "SUCCESS,$side,$symbol,$duration" >> "$results_file"
        echo "Request $id: $side order for $symbol completed in ${duration}s"
    else
        echo "FAILURE,$side,$symbol,$duration" >> "$results_file"
        echo "Request $id: $side order for $symbol FAILED: $response"
    fi
    
    # Apply delay if specified
    if (( $(echo "$delay > 0" | bc 2>/dev/null || echo 0) )); then
        sleep $delay
    fi
}

echo "Using sequential requests with background processes..."
# Process requests with background jobs (limited concurrency)
start_time=$(date +%s)
active_jobs=0

# Process in batches to maintain concurrency
for i in $(seq 1 $num_requests); do
    # Limit concurrent jobs
    while [ $(jobs -p | wc -l) -ge $concurrency ]; do
        sleep 0.1
    done
    
    process_request $i &
    
    # Apply delay if needed
    if (( $(echo "$delay > 0" | bc 2>/dev/null || echo 0) )); then
        sleep $delay
    fi
    
    # Show progress every 10% or every 50 requests, whichever is less
    if [ $((i % 50)) -eq 0 ]; then
        echo "Progress: $i/$num_requests requests"
    fi
done

# Wait for all jobs to finish
echo "Waiting for all requests to complete..."
wait
end_time=$(date +%s)

# Calculate total time
total_time=$((end_time - start_time))
if [ $total_time -eq 0 ]; then
    requests_per_second=$num_requests
else
    requests_per_second=$((num_requests / total_time))
fi

# Parse results file for statistics
total_successful=$(grep -c "SUCCESS" "$results_file" 2>/dev/null || echo 0)
total_failed=$(grep -c "FAILURE" "$results_file" 2>/dev/null || echo 0)
buy_count=$(grep -c "SUCCESS,buy" "$results_file" 2>/dev/null || echo 0)
sell_count=$(grep -c "SUCCESS,sell" "$results_file" 2>/dev/null || echo 0)

# Count by symbol
pranesh_count=$(grep -c "SUCCESS.*Pranesh" "$results_file" 2>/dev/null || echo 0)
superman_count=$(grep -c "SUCCESS.*Superman" "$results_file" 2>/dev/null || echo 0)
arnimzola_count=$(grep -c "SUCCESS.*Arnimzola" "$results_file" 2>/dev/null || echo 0)

# Calculate percentages
if [ $total_successful -gt 0 ]; then
    pranesh_pct=$((pranesh_count * 100 / total_successful))
    superman_pct=$((superman_count * 100 / total_successful))
    arnimzola_pct=$((arnimzola_count * 100 / total_successful))
else
    pranesh_pct=0
    superman_pct=0
    arnimzola_pct=0
fi

# Print results
echo
echo "=== Load Test Results ==="
echo "Timestamp: $(date '+%Y-%m-%dT%H:%M:%S')"
echo "Total requests: $num_requests"
echo "Successful requests: $total_successful"
echo "Failed requests: $total_failed"
echo "Buy orders: $buy_count"
echo "Sell orders: $sell_count"
echo "Elapsed time: $total_time seconds"
echo "Request rate: $requests_per_second requests/second"
echo
echo "Distribution by symbol:"
echo "  Pranesh: $pranesh_count orders (${pranesh_pct}%)"
echo "  Superman: $superman_count orders (${superman_pct}%)"
echo "  Arnimzola: $arnimzola_count orders (${arnimzola_pct}%)"

# Clean up
rm -rf "$tmp_dir"
