#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
WORKER_URL="${WORKER_URL:-http://localhost:8787}"
TIMEOUT=10

echo "=========================================="
echo "E2E Integration Tests for Tul Worker"
echo "=========================================="
echo "Worker URL: $WORKER_URL"
echo ""

# Function to print test results
print_result() {
    local test_name=$1
    local result=$2
    if [ "$result" -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $test_name: PASSED"
    else
        echo -e "${RED}✗${NC} $test_name: FAILED"
    fi
}

# Function to wait for worker to be ready
wait_for_worker() {
    echo -e "${YELLOW}Waiting for worker to be ready...${NC}"
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        # Try to connect to the worker - any response (even error) means it's running
        # We use a simple DNS query endpoint as health check
        local response=$(curl -s -m 3 -w "\n%{http_code}" "${WORKER_URL}/dns-query?name=example.com&type=A" -H "accept: application/dns-json" 2>&1)
        local http_code=$(echo "$response" | tail -n1)

        # If we get any HTTP response code (200, 404, 500, etc), the worker is responding
        if [[ "$http_code" =~ ^[0-9]{3}$ ]]; then
            echo -e "${GREEN}Worker is ready! (HTTP $http_code)${NC}"
            return 0
        fi

        attempt=$((attempt + 1))
        echo "Attempt $attempt/$max_attempts... (no response yet)"
        sleep 2
    done

    echo -e "${RED}Worker failed to start within timeout${NC}"
    echo "Last response: $response"
    return 1
}

# Test 1: Generic Web Proxy - Test accessing a simple webpage
test_generic_web_proxy() {
    echo ""
    echo "=========================================="
    echo "Test 1: Generic Web Proxy"
    echo "=========================================="

    # Test accessing example.com through the proxy
    local test_url="${WORKER_URL}/example.com"
    echo "Testing URL: $test_url"

    local response=$(curl -s -m $TIMEOUT -w "\n%{http_code}" "$test_url" 2>&1)
    local http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n-1)

    echo "HTTP Status Code: $http_code"
    echo "Response preview (first 200 chars):"
    echo "$body" | head -c 200
    echo ""

    # Check if we got a successful response
    if [ "$http_code" = "200" ] && echo "$body" | grep -q "Example Domain"; then
        echo -e "${GREEN}✓ Generic web proxy test PASSED${NC}"
        echo "  - Successfully proxied example.com"
        echo "  - Response contains expected content"
        return 0
    else
        echo -e "${RED}✗ Generic web proxy test FAILED${NC}"
        echo "  - Expected HTTP 200 and 'Example Domain' in response"
        echo "  - Got HTTP $http_code"
        return 1
    fi
}

# Test 2: DoH (DNS over HTTPS) - Test DNS query
test_doh() {
    echo ""
    echo "=========================================="
    echo "Test 2: DoH (DNS over HTTPS)"
    echo "=========================================="

    # Test DNS query for a common domain
    local test_url="${WORKER_URL}/dns-query?name=example.com&type=A"
    echo "Testing URL: $test_url"

    local response=$(curl -s -m $TIMEOUT -w "\n%{http_code}" \
        -H "accept: application/dns-json" \
        "$test_url" 2>&1)
    local http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n-1)

    echo "HTTP Status Code: $http_code"
    echo "Response:"
    echo "$body" | head -c 500
    echo ""

    # Check if we got a successful DNS response
    # DoH responses should contain "Answer" field with DNS records
    if [ "$http_code" = "200" ] && (echo "$body" | grep -q "Answer" || echo "$body" | grep -q "answer"); then
        echo -e "${GREEN}✓ DoH test PASSED${NC}"
        echo "  - Successfully queried DNS for example.com"
        echo "  - Response contains DNS answer"
        return 0
    else
        echo -e "${RED}✗ DoH test FAILED${NC}"
        echo "  - Expected HTTP 200 and DNS answer in response"
        echo "  - Got HTTP $http_code"
        return 1
    fi
}

# Main test execution
main() {
    local failed_tests=0

    # Wait for worker to be ready
    if ! wait_for_worker; then
        echo -e "${RED}Cannot proceed with tests - worker is not ready${NC}"
        exit 1
    fi

    # Run tests
    if ! test_generic_web_proxy; then
        failed_tests=$((failed_tests + 1))
    fi

    if ! test_doh; then
        failed_tests=$((failed_tests + 1))
    fi

    # Summary
    echo ""
    echo "=========================================="
    echo "Test Summary"
    echo "=========================================="

    if [ $failed_tests -eq 0 ]; then
        echo -e "${GREEN}All tests passed! ✓${NC}"
        exit 0
    else
        echo -e "${RED}$failed_tests test(s) failed ✗${NC}"
        exit 1
    fi
}

# Run main function
main
