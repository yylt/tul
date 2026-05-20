include .env

# 默认目标
.DEFAULT_GOAL := help

# 定义颜色输出
GREEN := \033[0;32m
RED := \033[0;31m
YELLOW := \033[0;33m
NC := \033[0m # No Color

.PHONY: help
help: ## 显示帮助信息(默认)
	@printf "${GREEN}可用命令:${NC}\n"
	@awk -F ':|##' '/^[a-zA-Z_-]+:.*##/ {printf "  ${YELLOW}%-20s${NC} %s\n", $$1, $$NF}' $(firstword $(MAKEFILE_LIST)) | sort

.PHONY: init
init: ## 初始化项目（安装依赖）
	@printf "${GREEN}初始化项目...${NC}\n"
	cargo fetch
	npm install -g wrangler

.PHONY: deploy
deploy: ## 部署到 Cloudflare Workers
	@printf "${GREEN}部署到 Cloudflare Workers...${NC}\n"
	@ npx wrangler deploy

.PHONY: dev
dev: ## 本地开发运行
	@printf "${GREEN}启动本地开发服务器...${NC}\n"
	@ npx wrangler dev -c .wrangler.dev.toml

.PHONY: lint
lint: ## 运行 Clippy 检查
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: lint-fix
lint-fix: ## 自动修复 Clippy 警告
	@printf "${GREEN}自动修复 Clippy 警告...${NC}\n"
	cargo clippy --all-targets --all-features --fix --allow-dirty

.PHONY: fmt
fmt: ## 格式化代码
	@printf "${GREEN}格式化代码...${NC}\n"
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## 检查代码格式
	@printf "${GREEN}检查代码格式...${NC}\n"
	cargo fmt --all -- --check

.PHONY: test
test: ## 运行测试
	@printf "${GREEN}运行测试...${NC}\n"
	cargo test

.PHONY: test-verbose
test-verbose: ## 运行测试（详细输出）
	@printf "${GREEN}运行测试（详细模式）...${NC}\n"
	cargo test -- --nocapture

.PHONY: clean
clean: ## 清理构建文件
	@printf "${GREEN}清理构建文件...${NC}\n"
	cargo clean
	rm -rf .wrangler

.PHONY: check
check: ## 检查代码（不生成二进制文件）
	@printf "${GREEN}检查代码...${NC}\n"
	cargo check

.PHONY: update
update: ## 更新依赖
	@printf "${GREEN}更新依赖...${NC}\n"
	cargo update

.PHONY: secret-put
secret-put: ## 添加 secret（使用方式：make secret-put NAME=SECRET_NAME）
	@if [ -z "$(NAME)" ]; then \
		printf "${RED}错误: 请指定 secret 名称，如: make secret-put NAME=MY_SECRET${NC}\n"; \
		exit 1; \
	fi
	@printf "${GREEN}添加 secret: $(NAME)...${NC}\n"
	@ npx wrangler secret put $(NAME)

.PHONY: secret-delete
secret-delete: ## 删除 secret（使用方式：make secret-delete NAME=SECRET_NAME）
	@if [ -z "$(NAME)" ]; then \
		printf "${RED}错误: 请指定 secret 名称，如: make secret-delete NAME=MY_SECRET${NC}\n"; \
		exit 1; \
	fi
	@printf "${GREEN}删除 secret: $(NAME)...${NC}\n"
	@ npx wrangler secret delete $(NAME)

# 组合命令
.PHONY: ci
ci: fmt-check lint test ## CI 流程（格式化检查、Clippy、测试）
	@printf "${GREEN}CI 检查通过！${NC}\n"

.PHONY: pre-commit
pre-commit: fmt lint test ## 提交前运行所有检查（格式化、Clippy、测试）
	@printf "${GREEN}所有检查通过！${NC}\n"
