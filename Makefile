include .env

.PHONY: deploy
deploy: ## deploy to cf workers
	@ npx wrangler deploy

.PHONY: dev
dev: ## run the project locally
	@ npx wrangler dev -c .wrangler.dev.toml

lint:
	cargo clippy --all-targets --all-features -- -D warnings