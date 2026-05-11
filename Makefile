include .env

.PHONY: deploy
deploy: ## deploy to cf workers
	@ npx wrangler deploy

.PHONY: dev
dev: ## run the project locally
	@ npx wrangler dev -c .wrangler.dev.toml

.PHONY: scheduler
scheduler: ## run the project locally with scheduled events
	# Self-test: run `make scheduler`, then open `/__scheduled` in the browser to trigger the cron handler.
	@ npx wrangler dev -c .wrangler.dev.toml --test-scheduled

lint:
	cargo clippy --all-targets --all-features -- -D warnings
