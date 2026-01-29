# Integration Tests

This directory contains end-to-end (e2e) integration tests for the Tul Cloudflare Worker.

## Test Coverage

The integration tests cover the following features:

1. **Generic Web Proxy** - Tests proxying web requests through the worker
   - Tests accessing `example.com` through the proxy
   - Tests accessing `api.github.com` through the proxy

2. **DoH (DNS over HTTPS)** - Tests DNS query functionality
   - Tests DNS A record queries
   - Validates DNS response format

## Running Tests Locally

### Prerequisites

- Rust toolchain installed
- Node.js and npm installed
- `worker-build` installed: `cargo install worker-build`
- `wrangler` installed: `npm install -g wrangler`

### Steps

1. Build the worker in development mode:
   ```bash
   worker-build --dev
   ```

2. Start the wrangler dev server:
   ```bash
   wrangler dev --config .wrangler.dev.toml --port 8787
   ```

3. In another terminal, run the integration tests:
   ```bash
   chmod +x integration/e2e-test.sh
   WORKER_URL=http://localhost:8787 ./integration/e2e-test.sh
   ```

## GitHub Actions

The e2e tests run automatically on pull requests to the `main` branch via the `.github/workflows/e2e-test.yml` workflow.

The workflow:
1. Builds the worker in development mode
2. Starts wrangler dev server
3. Runs the integration tests
4. Captures and displays wrangler logs for debugging
5. Uploads logs as artifacts

## Test Script

The `e2e-test.sh` script:
- Waits for the worker to be ready (up to 60 seconds)
- Runs each test with a 10-second timeout
- Provides colored output for easy reading
- Returns exit code 0 if all tests pass, 1 if any fail

## Environment Variables

- `WORKER_URL` - The URL of the worker to test (default: `http://localhost:8787`)
- `TIMEOUT` - Timeout in seconds for each test request (default: 10)

## Adding New Tests

To add new tests:

1. Add a new test function in `e2e-test.sh` following the pattern:
   ```bash
   test_new_feature() {
       echo "Test: New Feature"
       # Test implementation
       # Return 0 for success, 1 for failure
   }
   ```

2. Call the test function in the `main()` function

3. Update this README with the new test coverage
