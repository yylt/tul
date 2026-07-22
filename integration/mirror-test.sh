#!/bin/bash

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

WORKER_URL="${WORKER_URL:-http://localhost:8787}"

CRANE_VER="v0.21.7"
CRANE_TGZ="go-containerregistry_Linux_x86_64.tar.gz"
CRANE_URL="https://github.com/google/go-containerregistry/releases/download/${CRANE_VER}/${CRANE_TGZ}"
CRANE_DIR="/tmp/crane-${CRANE_VER}"
CRANE_BIN="${CRANE_DIR}/crane"

echo "=========================================="
echo "Docker Registry Mirror Tests"
echo "=========================================="
echo "Worker URL: $WORKER_URL"

# Extract host:port from WORKER_URL for crane (strip scheme)
REGISTRY_HOST=$(echo "$WORKER_URL" | sed 's|^https\?://||')
echo "Registry host: $REGISTRY_HOST"
echo ""

download_crane() {
    if [ -x "$CRANE_BIN" ]; then
        echo "Crane already installed at $CRANE_BIN"
        return 0
    fi

    echo -e "${YELLOW}Downloading crane ${CRANE_VER}...${NC}"
    mkdir -p "$CRANE_DIR"
    for i in 1 2 3; do
        if curl -fsSL --retry 2 --retry-delay 5 "$CRANE_URL" -o "${CRANE_DIR}/${CRANE_TGZ}" 2>/dev/null; then
            break
        fi
        echo -e "${YELLOW}Download attempt $i failed, retrying...${NC}"
        sleep 5
    done
    if [ ! -f "${CRANE_DIR}/${CRANE_TGZ}" ]; then
        echo -e "${RED}Failed to download crane after 3 attempts${NC}"
        exit 1
    fi
    tar xzf "${CRANE_DIR}/${CRANE_TGZ}" -C "$CRANE_DIR" crane
    chmod +x "$CRANE_BIN"
    echo -e "${GREEN}Crane installed at ${CRANE_BIN}${NC}"
}

print_result() {
    local test_name=$1
    local result=$2
    if [ "$result" -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $test_name: PASSED"
    else
        echo -e "${RED}✗${NC} $test_name: FAILED"
    fi
}

test_manifest() {
    echo ""
    echo "=========================================="
    echo "Test: crane manifest through proxy"
    echo "=========================================="

    local ref="${REGISTRY_HOST}/library/alpine:latest"
    echo "Reference: $ref"

    local output
    set +e
    output=$("$CRANE_BIN" manifest --insecure "$ref" 2>&1)
    local rc=$?
    set -e

    echo "Output (first 300 chars):"
    echo "$output" | head -c 300
    echo ""

    if [ $rc -eq 0 ] && echo "$output" | grep -q '"schemaVersion"'; then
        echo -e "${GREEN}✓ Manifest test PASSED${NC}"
        return 0
    else
        echo -e "${RED}✗ Manifest test FAILED${NC}"
        echo "  Exit code: $rc"
        return 1
    fi
}

test_digest() {
    echo ""
    echo "=========================================="
    echo "Test: crane digest through proxy"
    echo "=========================================="

    local ref="${REGISTRY_HOST}/library/alpine:latest"
    echo "Reference: $ref"

    local output
    set +e
    output=$("$CRANE_BIN" digest --insecure "$ref" 2>&1)
    local rc=$?
    set -e

    echo "Output: $output"

    if [ $rc -eq 0 ] && echo "$output" | grep -q 'sha256:'; then
        echo -e "${GREEN}✓ Digest test PASSED${NC}"
        return 0
    else
        echo -e "${RED}✗ Digest test FAILED${NC}"
        echo "  Exit code: $rc"
        return 1
    fi
}

test_config() {
    echo ""
    echo "=========================================="
    echo "Test: crane config through proxy"
    echo "=========================================="

    local ref="${REGISTRY_HOST}/library/busybox:latest"
    echo "Reference: $ref"

    local output
    set +e
    output=$("$CRANE_BIN" config --insecure "$ref" 2>&1)
    local rc=$?
    set -e

    echo "Output (first 300 chars):"
    echo "$output" | head -c 300
    echo ""

    if [ $rc -eq 0 ] && echo "$output" | grep -q '"os"'; then
        echo -e "${GREEN}✓ Config test PASSED${NC}"
        return 0
    else
        echo -e "${RED}✗ Config test FAILED${NC}"
        echo "  Exit code: $rc"
        return 1
    fi
}

main() {
    download_crane

    local failed_tests=0

    if ! test_manifest; then
        failed_tests=$((failed_tests + 1))
    fi

    if ! test_digest; then
        failed_tests=$((failed_tests + 1))
    fi

    if ! test_config; then
        failed_tests=$((failed_tests + 1))
    fi

    echo ""
    echo "=========================================="
    echo "Mirror Test Summary"
    echo "=========================================="

    if [ $failed_tests -eq 0 ]; then
        echo -e "${GREEN}All mirror tests passed! ✓${NC}"
        exit 0
    else
        echo -e "${RED}$failed_tests test(s) failed ✗${NC}"
        exit 1
    fi
}

main
