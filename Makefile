.PHONY: help dev build test clean install

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

install: ## Install frontend dependencies
	cd apps/frontend && bun install

dev-backend: ## Run backend in development mode
	moon run backend:dev

dev-frontend: ## Run frontend in development mode
	moon run frontend:dev

build: ## Build all projects
	moon run :build

build-backend: ## Build backend
	moon run backend:build

build-frontend: ## Build frontend
	moon run frontend:build

test: ## Run all tests
	moon run :test

check: ## Run moon check on all projects
	moon check --all

clean: ## Clean build artifacts
	rm -rf apps/backend/target
	rm -rf apps/frontend/.next
	rm -rf apps/frontend/out
	rm -rf apps/frontend/node_modules
	rm -rf .moon/cache

fmt-backend: ## Format backend code
	cd apps/backend && cargo fmt

clippy: ## Run clippy on backend
	moon run backend:clippy

lint-frontend: ## Lint frontend code
	moon run frontend:lint
