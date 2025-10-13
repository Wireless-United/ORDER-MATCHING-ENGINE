#!/usr/bin/env python3
import requests
import json
import random
import time
import concurrent.futures
import argparse
from datetime import datetime

# Configuration
BASE_URL = "http://localhost:3000"
SYMBOLS = ["Pranesh", "Superman", "Arnimzola"]
DEFAULT_REQUESTS = 1000
DEFAULT_CONCURRENCY = 50
DEFAULT_DELAY = 0.001  # Small delay between requests in seconds

def generate_order(symbol=None, side=None):
    """Generate a random order"""
    if symbol is None:
        symbol = random.choice(SYMBOLS)
    if side is None:
        side = random.choice(["buy", "sell"])
        
    order = {
        "symbol": symbol,
        "price": int(random.uniform(10.0, 1000.0)),  # API expects u64
        "qty": random.randint(1, 100)  # API expects u64 and the field name is 'qty' not 'quantity'
    }
    return order, side

def send_order(order, side):
    """Send an order to the matching engine"""
    endpoint = f"{BASE_URL}/{side}"
    try:
        response = requests.post(
            endpoint,
            headers={"Content-Type": "application/json"},
            data=json.dumps(order)
        )
        print(f"Response for {side} order: {response.status_code}, {response.text[:100]}")
        return {
            "success": response.status_code == 200, 
            "status_code": response.status_code,
            "response": response.text,
            "side": side,
            "symbol": order["symbol"]
        }
    except Exception as e:
        print(f"Error sending {side} order: {str(e)}")
        return {"success": False, "error": str(e), "side": side, "symbol": order["symbol"]}

def worker(i):
    """Worker function to send orders"""
    order, side = generate_order()
    return send_order(order, side)

def run_load_test(num_requests, concurrency, delay):
    """Run the load test with specified parameters"""
    print(f"Starting load test with {num_requests} requests at {concurrency} concurrency...")
    start_time = time.time()
    
    results = {
        "total": num_requests,
        "success": 0,
        "failed": 0,
        "buy_count": 0,
        "sell_count": 0,
        "symbols": {symbol: 0 for symbol in SYMBOLS}
    }
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = []
        for i in range(num_requests):
            futures.append(executor.submit(worker, i))
            if delay > 0:
                time.sleep(delay)
        
        for future in concurrent.futures.as_completed(futures):
            result = future.result()
            if result.get("success", False):
                results["success"] += 1
                results["symbols"][result["symbol"]] += 1
                if result["side"] == "buy":
                    results["buy_count"] += 1
                else:
                    results["sell_count"] += 1
            else:
                results["failed"] += 1
                
            # Print progress every 100 requests
            total_processed = results["success"] + results["failed"]
            if total_processed % 100 == 0:
                elapsed = time.time() - start_time
                rate = total_processed / elapsed if elapsed > 0 else 0
                print(f"Progress: {total_processed}/{num_requests} ({rate:.2f} req/sec)")
    
    elapsed = time.time() - start_time
    rate = num_requests / elapsed if elapsed > 0 else 0
    
    # Print results
    print("\n=== Load Test Results ===")
    print(f"Timestamp: {datetime.now().isoformat()}")
    print(f"Total requests: {num_requests}")
    print(f"Successful requests: {results['success']}")
    print(f"Failed requests: {results['failed']}")
    print(f"Buy orders: {results['buy_count']}")
    print(f"Sell orders: {results['sell_count']}")
    print(f"Elapsed time: {elapsed:.2f} seconds")
    print(f"Request rate: {rate:.2f} requests/second")
    print("\nDistribution by symbol:")
    for symbol, count in results["symbols"].items():
        percentage = (count / results["success"] * 100) if results["success"] > 0 else 0
        print(f"  {symbol}: {count} orders ({percentage:.1f}%)")
    
    return results

def main():
    parser = argparse.ArgumentParser(description='Load test for Order Matching Engine')
    parser.add_argument('-n', '--num-requests', type=int, default=DEFAULT_REQUESTS,
                        help=f'Number of requests to send (default: {DEFAULT_REQUESTS})')
    parser.add_argument('-c', '--concurrency', type=int, default=DEFAULT_CONCURRENCY,
                        help=f'Number of concurrent requests (default: {DEFAULT_CONCURRENCY})')
    parser.add_argument('-d', '--delay', type=float, default=DEFAULT_DELAY,
                        help=f'Delay between requests in seconds (default: {DEFAULT_DELAY})')
    parser.add_argument('-i', '--infinite', action='store_true',
                        help='Run in infinite mode (ignores num-requests)')
    
    args = parser.parse_args()
    
    # Health check before starting
    try:
        response = requests.post(f"{BASE_URL}/health", json={})
        if response.status_code != 200:
            print(f"Health check failed: {response.status_code}")
            return
        print("Health check passed! Starting load test...")
    except Exception as e:
        print(f"Error connecting to server: {e}")
        return
    
    if args.infinite:
        print("Running in infinite mode. Press Ctrl+C to stop.")
        count = 0
        try:
            while True:
                run_load_test(100, args.concurrency, args.delay)
                count += 100
                print(f"Sent {count} requests so far. Continuing...")
        except KeyboardInterrupt:
            print("\nLoad test stopped by user.")
    else:
        run_load_test(args.num_requests, args.concurrency, args.delay)

if __name__ == "__main__":
    main()
