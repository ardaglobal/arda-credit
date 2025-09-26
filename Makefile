# Demo vApp Makefile
# Provides convenient shortcuts for common development tasks

REGISTRY ?= ghcr.io
OWNER ?= ardaglobal
IMAGE_NAME ?= arda-credit
DOCKER_TAG ?= $(shell whoami)-dev
PLATFORM ?= linux/amd64
APPNAME ?= arda-credit


## help: Get more info on make commands.
.PHONY: help
help: Makefile
	@echo " Choose a command to run in '$(APPNAME)':"
	@sed -n 's/^##//p' $< | column -t -s ':' | sed -e 's/^/ /'
	@echo ""

## install: Install all dependencies
.PHONY: install
install:
	./install-dependencies.sh

## build-program: Build SP1 program
.PHONY: build-program
build-program:
	cd program && cargo prove build --output-directory ../build

## fmt: Format code
.PHONY: fmt
fmt: yamllint-fix actionlint-fix docker-lint-fix
	@if [ "$(CHECK)" = "1" ]; then \
		echo ""; \
		echo "Running formatters with --check"; \
		echo ""; \
		cargo fmt -- --check; \
		taplo fmt --check; \
	else \
		echo ""; \
		echo "Formatting code"; \
		echo ""; \
		cargo fmt; \
		taplo fmt; \
	fi

## lint: Lint code
.PHONY: lint
lint: yamllint actionlint docker-lint deny lint-rust

## lint-rust: Lint only Rust code (excluding tests)
.PHONY: lint-rust
lint-rust:
	$(MAKE) fmt CHECK=1
	SQLX_OFFLINE=true cargo clippy --lib --bins -- --cap-lints=warn -A warnings -D clippy::pedantic -D clippy::nursery -D clippy::style -D clippy::complexity -D clippy::perf -D clippy::suspicious -D clippy::correctness

## env: Copy .env.example to .env
.PHONY: env
env:
	cp .env.example .env
	@echo "✅ Environment file created. Edit .env to add your SINDRI_API_KEY"

## deploy: Deploy circuit to Sindri
.PHONY: deploy
deploy:
	./deploy-circuit.sh

## up: Start services (uses pre-built image)
.PHONY: up
up:
	docker compose up -d
	@echo "✅ Backgrond Services starting"

## down: Stop all services
.PHONY: down
down:
	docker compose down -v

## docker-build: Build image locally
.PHONY: docker-build
docker-build:
	@echo "Building Docker image locally..."
	@echo "Image: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"
	@echo "Platform: $(PLATFORM)"
	@echo ""
	docker build --platform $(PLATFORM) -t $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG) .
	@echo "✅ Image built successfully!"
	@echo "🐳 Image: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"
	@echo "🏗️  Platform: $(PLATFORM)"

## docker-push: Build and push image to GitHub registry
# Docker Configuration:
#   PLATFORM=linux/amd64  Build for x86_64 (default)
#   PLATFORM=linux/arm64  Build for ARM64
#   Example: make docker-push PLATFORM=linux/amd64
.PHONY: docker-push
docker-push: docker-build
	@echo "Pushing Docker image..."
	@echo "Registry: $(REGISTRY)"
	docker push $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)
	@echo "✅ Image pushed successfully!"
	@echo "🚀 Published: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"

## docker-lint: Lint Dockerfiles using hadolint
.PHONY: docker-lint
docker-lint:
	@echo "🔍 Linting Dockerfiles with hadolint..."
	@command -v hadolint >/dev/null 2>&1 || { echo "❌ hadolint not found. Run 'make install' to install dependencies."; exit 1; }
	hadolint Dockerfile
	@echo "✅ Dockerfile linting completed"

## docker-lint-fix: Lint and show suggestions to fix Dockerfile issues
.PHONY: docker-lint-fix
docker-lint-fix:
	@echo "🔧 Linting Dockerfiles with detailed suggestions..."
	@command -v hadolint >/dev/null 2>&1 || { echo "❌ hadolint not found. Run 'make install' to install dependencies."; exit 1; }
	hadolint --format tty Dockerfile || true
	@echo "💡 See https://github.com/hadolint/hadolint/wiki/DL4006 for rule explanations"

## yamllint: Lint YAML files using yamllint
.PHONY: yamllint
yamllint:
	@echo "🔍 Linting YAML files with yamllint..."
	@command -v yamllint >/dev/null 2>&1 || { echo "❌ yamllint not found. Run 'make install' to install dependencies."; exit 1; }
	yamllint .
	@echo "✅ YAML linting completed"

## yamllint-fix: Lint YAML files with verbose output for fixing issues
.PHONY: yamllint-fix
yamllint-fix:
	@echo "🔧 Linting YAML files with detailed output..."
	@command -v yamllint >/dev/null 2>&1 || { echo "❌ yamllint not found. Run 'make install' to install dependencies."; exit 1; }
	yamllint --format parsable . || true
	@echo "💡 Fix YAML issues based on the output above"

## actionlint: Lint GitHub Actions workflows using actionlint
.PHONY: actionlint
actionlint:
	@echo "🔍 Linting GitHub Actions workflows with actionlint..."
	@command -v actionlint >/dev/null 2>&1 || { echo "❌ actionlint not found. Run 'make install' to install dependencies."; exit 1; }
	actionlint -config-file .actionlint.yaml -ignore SC2086
	@echo "✅ GitHub Actions linting completed"

## actionlint-fix: Lint GitHub Actions workflows with verbose output for fixing issues
.PHONY: actionlint-fix
actionlint-fix:
	@echo "🔧 Linting GitHub Actions workflows with detailed output..."
	@command -v actionlint >/dev/null 2>&1 || { echo "❌ actionlint not found. Run 'make install' to install dependencies."; exit 1; }
	actionlint -config-file .actionlint.yaml -ignore SC2086 -verbose || true
	@echo "💡 Fix GitHub Actions issues based on the output above"

## test: Run all tests
.PHONY: test
test:
	docker compose up postgres -d
	@sleep 2
	DATABASE_URL="postgresql://postgres:password@localhost:5432/arda_credit_db?sslmode=disable" sqlx migrate run --source db/migrations
	DATABASE_URL="postgresql://postgres:password@localhost:5432/arda_credit_db?sslmode=disable" cargo test -- --test-threads=1
	docker compose down -v

## cover: Generate code coverage report
.PHONY: cover
cover:
	@echo "🔍 Generating code coverage report..."
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "❌ cargo-llvm-cov not found. Run 'make install' to install dependencies."; exit 1; }
	@echo "🚀 Starting PostgreSQL database..."
	@docker compose up postgres -d
	@sleep 2
	@echo "📦 Running database migrations..."
	@sqlx migrate run --source db/migrations
	@echo "📊 Generating coverage reports..."
	@mkdir -p target/llvm-cov
	@echo "  📄 Generating HTML report..."
	cargo llvm-cov --workspace --all-features --html -- --test-threads=1
	@echo "  📋 Generating JSON report..."
	cargo llvm-cov --workspace --all-features --json --output-path target/llvm-cov/coverage.json -- --test-threads=1
	@echo "  📝 Generating summary report..."
	cargo llvm-cov --workspace --all-features --summary-only -- --test-threads=1 > target/llvm-cov/coverage-summary.txt
	@echo "🛑 Stopping PostgreSQL database..."
	@docker compose down -v
	@echo "🎉 Coverage reports generated successfully!"
	@echo ""
	@echo "📂 Reports available at:"
	@echo "  📄 HTML Report:    target/llvm-cov/html/index.html"
	@echo "  📋 JSON Data:      target/llvm-cov/coverage.json"
	@echo "  📝 Summary:        target/llvm-cov/coverage-summary.txt"
	@echo ""
	@echo "=== Coverage Summary ==="
	@if [ -f "target/llvm-cov/coverage-summary.txt" ]; then \
		cat target/llvm-cov/coverage-summary.txt; \
	fi
	@echo ""
	@echo "💡 Open target/llvm-cov/html/index.html in your browser to view the detailed HTML report"

## run: Local SP1 unit testing (fast ~3.5s Core proofs)
.PHONY: run
run:
	cargo run --release

# =============================================================================
# ZK Proof Benchmarking Commands (requires SINDRI_API_KEY)
# =============================================================================

NUM_PROOFS=1
NUM_TRANSACTIONS=100

## bench-zk: Run a zk benchmark test. Usage: make bench-zk NUM_PROOFS=50 NUM_TRANSACTIONS=500
.PHONY: bench-zk
bench-zk:
	@echo ""
	@echo "Prerequisites:"
	@echo "  • SINDRI_API_KEY set in .env"
	@echo "  • Update sindri.json provingScheme to compressed"
	@echo "  • Circuit deployed: make deploy"
	@echo ""
	@TOTAL_TXS=$$(($(NUM_PROOFS) * $(NUM_TRANSACTIONS))); \
	echo "🎛️  Running benchmark test..."; \
	echo "Configuration: $(NUM_PROOFS) proofs × $(NUM_TRANSACTIONS) transactions = $$TOTAL_TXS total transactions"; \
	echo ""
	NUM_PROOFS=$(NUM_PROOFS) NUM_TRANSACTIONS=$(NUM_TRANSACTIONS) cargo run --release --package arda-credit --features sindri,benchmark

## cli: CLI client (requires API server running)
.PHONY: cli
cli:
	@cargo run --bin cli -- $(ARGS)

## server: Start API server locally (in debug mode with verbose logging) with deposit monitor and withdrawal executor
.PHONY: server
server: up
	@sleep 2
	@echo "Running database migrations..."
	sqlx migrate run --source db/migrations
	@echo "Starting deposit monitor in background..."
	@SQLX_OFFLINE=true cargo run -p ethereum-client --bin deposit_monitor --release -- \
		--database-url postgresql://postgres:password@127.0.0.1:5432/arda_credit_db \
		--ethereum-rpc-url $$ETHEREUM_RPC_URL \
		--contract-address $$ETHEREUM_CONTRACT_ADDRESS \
		> deposit_monitor.log 2>&1 &
	@echo "Deposit monitor started (logs: deposit_monitor.log)"
	@echo "Starting withdrawal executor in background..."
	@SQLX_OFFLINE=true cargo run -p ethereum-client --bin withdrawal_executor --release -- \
		--database-url postgresql://postgres:password@127.0.0.1:5432/arda_credit_db \
		--ethereum-rpc-url $$ETHEREUM_RPC_URL \
		--contract-address $$ETHEREUM_CONTRACT_ADDRESS \
		--private-key $$ETHEREUM_PRIVATE_KEY \
		--poll-interval 30 \
		--batch-size 10 \
		> withdrawal_executor.log 2>&1 &
	@echo "Withdrawal executor started (logs: withdrawal_executor.log)"
	@echo "Starting API server..."
	RUST_LOG=debug cargo run --bin server

## server-stop: Stop background services (deposit monitor and withdrawal executor)
.PHONY: server-stop
server-stop:
	@echo "🛑 Stopping background services..."
	@pkill -f "deposit_monitor" || echo "  No deposit monitor process found"
	@pkill -f "withdrawal_executor" || echo "  No withdrawal executor process found"
	@echo "✅ Background services stopped"

## server-help: Show required environment variables for server
.PHONY: server-help
server-help:
	@echo "Server Environment Variables"
	@echo "==========================="
	@echo ""
	@echo "Required for Deposit Monitor and Withdrawal Executor:"
	@echo "  ETHEREUM_RPC_URL          - Ethereum RPC endpoint URL"
	@echo "  ETHEREUM_CONTRACT_ADDRESS - Arda Credit contract address"
	@echo "  ETHEREUM_PRIVATE_KEY      - Private key for withdrawal transactions (hex format)"
	@echo ""
	@echo "Optional Database (uses default if not set):"
	@echo "  DATABASE_URL              - PostgreSQL connection string"
	@echo "                               (default: postgresql://postgres:password@127.0.0.1:5432/arda_credit_db)"
	@echo ""
	@echo "Example Usage:"
	@echo "  export ETHEREUM_RPC_URL='https://eth-sepolia.g.alchemy.com/v2/your-api-key'"
	@echo "  export ETHEREUM_CONTRACT_ADDRESS='0x45F9Dff1f6c75C9Ab28e8c5167F2461A2B295c1A'"
	@echo "  export ETHEREUM_PRIVATE_KEY='your-private-key-without-0x-prefix'"
	@echo "  make server"
	@echo ""
	@echo "Or set them in your .env file and run:"
	@echo "  export \$$(cat .env | grep -v '^#' | xargs) && make server"
	@echo ""
	@echo "Log Files:"
	@echo "  deposit_monitor.log       - Deposit monitor service logs"
	@echo "  withdrawal_executor.log   - Withdrawal executor service logs"
	@echo ""
	@echo "Stop Services:"
	@echo "  make server-stop          - Stop background services"

## server-logs: Show logs from background services
.PHONY: server-logs
server-logs:
	@echo "=== Deposit Monitor Logs ==="
	@if [ -f "deposit_monitor.log" ]; then \
		tail -20 deposit_monitor.log; \
	else \
		echo "❌ deposit_monitor.log not found"; \
	fi
	@echo ""
	@echo "=== Withdrawal Executor Logs ==="
	@if [ -f "withdrawal_executor.log" ]; then \
		tail -20 withdrawal_executor.log; \
	else \
		echo "❌ withdrawal_executor.log not found"; \
	fi
	@echo ""
	@echo "💡 Use 'tail -f deposit_monitor.log' or 'tail -f withdrawal_executor.log' for live monitoring"

## forge-build: Build smart contracts
.PHONY: forge-build
forge-build:
	@echo "🔨 Building ARDA smart contracts..."
	cd contracts && forge build
	@echo "✅ ARDA smart contracts built successfully"

## forge-test: Run smart contract tests
.PHONY: forge-test
forge-test:
	@echo "🧪 Running ARDA smart contract tests..."
	cd contracts && forge test
	@echo "✅ ARDA smart contract tests completed"

## deploy-contract: Deploy Arda Credit smart contract
# Required environment variables:
#   ETHEREUM_RPC_URL     - RPC endpoint URL (e.g., Alchemy, Infura)
#   ETHEREUM_WALLET_PRIVATE_KEY         - Private key for deployment (without 0x prefix)
#   VERIFIER_CONTRACT_ADDRESS - SP1 verifier contract address
#   PROGRAM_VKEY        - Program verification key
.PHONY: deploy-contract
deploy-contract:
	@echo "🚀 Deploying ARDA smart contract..."
	@echo "🔍 Checking required environment variables..."
	@if [ -z "$$ETHEREUM_RPC_URL" ]; then \
		echo "❌ ETHEREUM_RPC_URL is required"; \
		exit 1; \
	fi
	@if [ -z "$$ETHEREUM_WALLET_PRIVATE_KEY" ]; then \
		echo "❌ ETHEREUM_WALLET_PRIVATE_KEY is required"; \
		exit 1; \
	fi
	@if [ -z "$$VERIFIER_CONTRACT_ADDRESS" ]; then \
		echo "❌ VERIFIER_CONTRACT_ADDRESS is required"; \
		exit 1; \
	fi
	@if [ -z "$$PROGRAM_VKEY" ]; then \
		echo "❌ PROGRAM_VKEY is required"; \
		exit 1; \
	fi
	@echo "✅ All environment variables are set"
	@echo ""
	@echo "🚀 Deploying contract..."
	@echo "📡 RPC URL: $$ETHEREUM_RPC_URL"
	@echo "🔑 Verifier: $$VERIFIER_CONTRACT_ADDRESS"
	@echo "🗝️  Program VKey: $$PROGRAM_VKEY"
	@echo ""
	cd contracts && forge create src/ARDA.sol:ARDA \
		--broadcast \
		--rpc-url $$ETHEREUM_RPC_URL \
		--private-key $$ETHEREUM_WALLET_PRIVATE_KEY \
		--constructor-args $$VERIFIER_CONTRACT_ADDRESS $$PROGRAM_VKEY
	@echo "✅ Contract deployed successfully!"

## deploy-contract-help: Show deployment command usage
.PHONY: deploy-contract-help
deploy-contract-help:
	@echo "ARDA Smart Contract Deployment Help"
	@echo "==================================="
	@echo ""
	@echo "Required Environment Variables:"
	@echo "  ETHEREUM_RPC_URL          - Ethereum RPC endpoint"
	@echo "  ETHEREUM_WALLET_PRIVATE_KEY              - Deployment wallet private key (no 0x prefix)"
	@echo "  VERIFIER_CONTRACT_ADDRESS - SP1 verifier contract address"
	@echo "  PROGRAM_VKEY             - Program verification key"
	@echo ""
	@echo "Example Usage:"
	@echo "  export ETHEREUM_RPC_URL='https://eth-mainnet.g.alchemy.com/v2/your-api-key'"
	@echo "  export ETHEREUM_WALLET_PRIVATE_KEY='your-private-key-without-0x-prefix'"
	@echo "  export VERIFIER_CONTRACT_ADDRESS='0x1234...'"
	@echo "  export PROGRAM_VKEY='0xabcd...'"
	@echo "  make deploy-contract"
	@echo ""
	@echo "Or set them in your .env file and run:"
	@echo "  export \$$(cat .env | grep -v '^#' | xargs) && make deploy-contract"

## clean-docker: Clean up Docker resources
.PHONY: clean-docker
clean-docker:
	docker compose down -v
	@echo "🧹 Cleaning Docker resources (preserving Kind clusters)..."
	@# Preserve Kind networks and containers during cleanup
	@if kind get clusters 2>/dev/null | grep -q .; then \
		echo "ℹ️  Found active Kind clusters, preserving Kind resources"; \
		docker system prune -f --filter "label!=io.x-k8s.kind.cluster" --filter "label!=io.x-k8s.kind.role"; \
	else \
		echo "ℹ️  No Kind clusters found, running full cleanup"; \
		docker system prune -f -a; \
	fi
	@echo "✅ Docker resources cleaned up"

## kind-docker-clean: Clean Docker resources while preserving ALL Kind clusters
.PHONY: kind-docker-clean
kind-docker-clean:
	docker compose down -v
	@echo "🧹 Safely cleaning Docker resources (preserving ALL Kind resources)..."
	@# Only clean non-Kind resources
	docker container prune -f --filter "label!=io.x-k8s.kind.cluster"
	docker image prune -f --filter "label!=io.x-k8s.kind.cluster"
	docker volume prune -f --filter "label!=io.x-k8s.kind.cluster"
	@# Preserve all Kind networks
	@for network in $$(docker network ls --filter "name=kind" --format "{{.Name}}"); do \
		echo "ℹ️  Preserving Kind network: $$network"; \
	done
	docker network prune -f --filter "name!=kind*"
	@echo "✅ Docker resources safely cleaned up (Kind clusters preserved)"

## clean-sqlx: Clean up SQLx resources
.PHONY: clean-sqlx
clean-sqlx:
	find . -type d -name ".sqlx" -exec rm -rf {} +
	@echo "✅ SQLx resources cleaned up"

## clean-builds: Clean up build artifacts
.PHONY: clean-builds
clean-builds:
	rm -rf target
	rm -rf build
	rm -rf ADS
	@echo "✅ Build artifacts cleaned up"

## clean: Clean up all resources
.PHONY: clean
clean: clean-docker clean-sqlx clean-builds
	@echo "✅ All resources cleaned up"

## helm-deps: Update Helm chart dependencies
.PHONY: helm-deps
helm-deps:
	@echo "📦 Updating Helm chart dependencies..."
	@command -v helm >/dev/null 2>&1 || { echo "❌ helm not found. Please install Helm 3.8+"; exit 1; }
	@if [ ! -f "helm/charts/arda-credit/Chart.lock" ] || [ "helm/charts/arda-credit/Chart.yaml" -nt "helm/charts/arda-credit/Chart.lock" ]; then \
		echo "🔄 Chart.yaml newer than Chart.lock or Chart.lock missing, updating dependencies..."; \
		helm repo add bitnami https://charts.bitnami.com/bitnami >/dev/null 2>&1 || true; \
		helm repo update >/dev/null 2>&1; \
		helm dependency update ./helm/charts/arda-credit; \
		echo "✅ Helm dependencies updated successfully"; \
	else \
		echo "✅ Helm dependencies are up to date"; \
	fi

## helm-upgrade: Upgrade Arda Credit Helm deployment (preserves PostgreSQL by default)
# Environment variables (optional):
#   HELM_NAMESPACE      - Kubernetes namespace (default: arda-credit)
#   HELM_VALUES_FILE    - Custom values file path
#   UPGRADE_POSTGRES    - Set to "true" to include PostgreSQL in upgrade (default: false)
.PHONY: helm-upgrade
helm-upgrade: helm-deps
	@echo "🔄 Upgrading Arda Credit Helm deployment..."
	@command -v helm >/dev/null 2>&1 || { echo "❌ helm not found. Please install Helm 3.8+"; exit 1; }
	@command -v kubectl >/dev/null 2>&1 || { echo "❌ kubectl not found. Please install kubectl"; exit 1; }
	@NAMESPACE=$${HELM_NAMESPACE:-arda-credit}; \
	VALUES_FILE=$${HELM_VALUES_FILE:-}; \
	UPGRADE_POSTGRES=$${UPGRADE_POSTGRES:-false}; \
	echo "📍 Target namespace: $$NAMESPACE"; \
	if [ "$$UPGRADE_POSTGRES" = "true" ]; then \
		echo "⚠️  PostgreSQL will be included in upgrade"; \
	else \
		echo "🛡️  PostgreSQL will be preserved (use UPGRADE_POSTGRES=true to include)"; \
	fi; \
	if helm status arda-credit -n $$NAMESPACE >/dev/null 2>&1; then \
		if [ "$$UPGRADE_POSTGRES" = "true" ]; then \
			echo "⚠️  Including PostgreSQL in upgrade - this may restart the database"; \
			UPGRADE_CMD="helm upgrade arda-credit ./helm/charts/arda-credit -n $$NAMESPACE --reuse-values"; \
		else \
			echo "🛡️  Preserving PostgreSQL completely - only upgrading app components"; \
			UPGRADE_CMD="helm upgrade arda-credit ./helm/charts/arda-credit -n $$NAMESPACE --reuse-values --no-hooks"; \
		fi; \
		if [ -n "$$VALUES_FILE" ]; then \
			echo "📊 Using custom values file: $$VALUES_FILE"; \
			UPGRADE_CMD="$$UPGRADE_CMD -f $$VALUES_FILE"; \
		fi; \
		eval $$UPGRADE_CMD; \
		echo "✅ Helm upgrade completed successfully"; \
	else \
		echo "❌ Release 'arda-credit' not found in namespace $$NAMESPACE"; \
		echo "💡 Use 'helm install' for initial deployment"; \
		exit 1; \
	fi

## initDB: Initialize database (start, migrate, generate cache, stop)
.PHONY: initDB
initDB:
	@echo "🔧 Initializing database with SQLx offline mode support..."
	@echo ""
	@# Check if DATABASE_URL is set, use default if not
	@if [ -z "$$DATABASE_URL" ]; then \
		echo "ℹ️  DATABASE_URL not set, using default..."; \
		export DATABASE_URL="postgresql://postgres:password@localhost:5432/arda_credit_db"; \
	fi
	@echo "📍 Using database: $$DATABASE_URL"
	@echo ""
	@# Start PostgreSQL database
	@echo "🚀 Starting PostgreSQL database..."
	@docker compose up postgres -d
	@echo ""
	@# Wait for PostgreSQL to be ready
	@echo "⏳ Waiting for PostgreSQL to be ready..."
	@sleep 8
	@# Check database connectivity
	@echo "🏥 Checking database connectivity..."
	@if ! pg_isready -h localhost -p 5432 -U postgres >/dev/null 2>&1; then \
		echo "❌ PostgreSQL is not ready. Please check if it's running and accessible."; \
		exit 1; \
	fi
	@echo ""
	@# Run database migrations
	@echo "📦 Running database migrations..."
	@cd db && DATABASE_URL="postgresql://postgres:password@localhost:5432/arda_credit_db" sqlx migrate run
	@echo ""
	@# Generate SQLx cache
	@echo "💾 Generating SQLx cache for offline mode..."
	@DATABASE_URL="postgresql://postgres:password@localhost:5432/arda_credit_db" cargo sqlx prepare --workspace
	@echo ""
	@# Stop PostgreSQL database
	@echo "🛑 Stopping PostgreSQL database..."
	@docker compose down postgres -v
	@echo ""
	@echo "✅ Database initialization complete!"
	@echo ""
	@echo "💡 You can now use 'SQLX_OFFLINE=true cargo check' without a database connection."
	@echo "📝 The .sqlx/ directory has been updated and should be committed to version control."

## setup-hooks: Setup git hooks for pre-commit linting and formatting
.PHONY: setup-hooks
setup-hooks:
	@echo "🔧 Setting up git hooks..."
	@git config core.hooksPath .githooks
	@chmod +x .githooks/pre-commit
	@chmod +x .githooks/pre-push
	@echo "✅ Git hooks configured successfully"
	@echo "💡 Pre-commit hook will run 'make fmt' and 'make lint' automatically"
	@echo "💡 Pre-push hook will run 'make test' automatically"

## helm-uninstall: Remove Arda Credit from Kubernetes
.PHONY: helm-uninstall
helm-uninstall:
	@echo "🗑️  Uninstalling Arda Credit from Kubernetes..."
	@NAMESPACE=$${HELM_NAMESPACE:-arda-credit}; \
	echo "📍 Target namespace: $$NAMESPACE"; \
	if helm status arda-credit -n $$NAMESPACE >/dev/null 2>&1; then \
		helm uninstall arda-credit -n $$NAMESPACE; \
		echo "✅ Helm uninstall completed successfully"; \
		echo "💡 Note: Namespace and PVCs preserved"; \
	else \
		echo "ℹ️  Release 'arda-credit' not found in namespace $$NAMESPACE"; \
	fi

## setup: Complete setup from scratch
.PHONY: setup
setup: install env initDB setup-hooks
	@echo ""
	@echo "🎉 Setup complete! Next steps:"
	@echo "1. Edit .env and add your SINDRI_API_KEY"
	@echo "2. Run: make deploy                    # Deploy circuit to Sindri"
	@echo "3. Run: make up                        # Start services"
	@echo "4. Run: make deploy-contract-help      # For smart contract deployment"

## deny: Run deny
.PHONY: deny
deny:
	cargo deny check

## kind-setup: Setup Kind cluster for local e2e testing
# This creates a local Kubernetes cluster with Arda Credit deployed
# Uses locally built images for debugging and development
# Requires GHCR_USERNAME and GHCR_TOKEN in .env for private image pulls
.PHONY: kind-setup
kind-setup:
	@echo "🚀 Setting up Kind cluster for e2e testing..."
	@echo "💡 Ensure GHCR_USERNAME and GHCR_TOKEN are set in .env for private image pulls"
	@command -v kind >/dev/null 2>&1 || { echo "❌ kind not found. Run 'make install' to install dependencies."; exit 1; }
	@command -v kubectl >/dev/null 2>&1 || { echo "❌ kubectl not found. Run 'make install' to install dependencies."; exit 1; }
	@command -v helm >/dev/null 2>&1 || { echo "❌ helm not found. Run 'make install' to install dependencies."; exit 1; }
	cd helm/e2e && ./scripts/setup-cluster.sh
	@echo ""
	@echo "✅ Kind cluster setup complete!"
	@echo "🌐 Frontend: http://localhost:3000"
	@echo "🔧 API: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

## kind-teardown: Teardown Kind cluster
# Removes the local Kubernetes cluster and cleans up resources
.PHONY: kind-teardown
kind-teardown:
	@echo "🗑️  Tearing down Kind cluster..."
	cd helm/e2e && ./scripts/teardown-cluster.sh

## kind-status: Show Kind cluster status
# Display current status of the local e2e environment
.PHONY: kind-status
kind-status:
	@echo "📊 Kind Cluster Status:"
	@if kind get clusters | grep -q "arda-credit-e2e"; then \
		echo "✅ Cluster 'arda-credit-e2e' is running"; \
		echo ""; \
		echo "📦 Arda Credit Pods (arda-credit namespace):"; \
		kubectl get pods -n arda-credit --context=kind-arda-credit-e2e 2>/dev/null || echo "❌ No pods found in arda-credit namespace"; \
		echo ""; \
		echo "🔗 Arda Credit Services (arda-credit namespace):"; \
		kubectl get services -n arda-credit --context=kind-arda-credit-e2e 2>/dev/null || echo "❌ No services found in arda-credit namespace"; \
		echo ""; \
		echo "📊 All Namespaces:"; \
		kubectl get pods --all-namespaces --context=kind-arda-credit-e2e 2>/dev/null | head -10 || echo "❌ Cannot connect to cluster"; \
	else \
		echo "❌ Cluster 'arda-credit-e2e' not found"; \
		echo "💡 Run 'make kind-setup' to create the cluster"; \
	fi

## kind-logs-server: Show logs from Arda Credit server
.PHONY: kind-logs-server
kind-logs-server:
	@echo "=== Server Logs ==="
	@kubectl logs --context=kind-arda-credit-e2e -n arda-credit -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=server --tail=50 2>/dev/null || echo "❌ No server logs available"

## kind-logs-app: Show logs from Arda Credit app
.PHONY: kind-logs-app
kind-logs-app:
	@echo "=== App Logs ==="
	@kubectl logs --context=kind-arda-credit-e2e -n arda-credit -l app.kubernetes.io/name=arda-credit,app.kubernetes.io/component=app --tail=50 2>/dev/null || echo "❌ No app logs available"

## kind-logs-postgres: Show logs from PostgreSQL
.PHONY: kind-logs-postgres
kind-logs-postgres:
	@echo "=== PostgreSQL Logs ==="
	@kubectl logs --context=kind-arda-credit-e2e -n arda-credit -l app.kubernetes.io/name=postgresql --tail=20 2>/dev/null || echo "❌ No PostgreSQL logs available"

## kind-logs: Show logs from all Kind cluster services
.PHONY: kind-logs
kind-logs: kind-logs-server kind-logs-app kind-logs-postgres

## kind-restart: Restart Kind cluster services
# Restart Arda Credit deployments (keeps cluster and data)
.PHONY: kind-restart
kind-restart:
	@echo "🔄 Restarting Arda Credit services..."
	@kubectl rollout restart deployment/arda-credit-server --context=kind-arda-credit-e2e -n arda-credit
	@kubectl rollout restart deployment/arda-credit-app --context=kind-arda-credit-e2e -n arda-credit
	@echo "⏳ Waiting for rollout to complete..."
	@kubectl rollout status deployment/arda-credit-server --context=kind-arda-credit-e2e -n arda-credit --timeout=120s
	@kubectl rollout status deployment/arda-credit-app --context=kind-arda-credit-e2e -n arda-credit --timeout=60s
	@echo "✅ Services restarted successfully"

## kind-rebuild: Rebuild images and restart Kind services
# Rebuilds local images, loads them into Kind, and restarts services
.PHONY: kind-rebuild
kind-rebuild:
	@echo "🔨 Rebuilding images and restarting services..."
	@echo "Building new image..."
	@docker build -t localhost/arda-credit:local-dev .
	@echo "Loading image into Kind cluster..."
	@kind load docker-image localhost/arda-credit:local-dev --name=arda-credit-e2e
	@docker tag localhost/arda-credit:local-dev localhost/arda-credit-app:local-dev
	@kind load docker-image localhost/arda-credit-app:local-dev --name=arda-credit-e2e
	@echo "Restarting services..."
	@$(MAKE) kind-restart
	@echo "✅ Rebuild and restart complete!"

## kind-test: Run deployment validation tests
# Test current Kind cluster deployment health
.PHONY: kind-test
kind-test:
	@echo "🧪 Running deployment validation tests..."
	cd helm/e2e && ./tests/test-deployment.sh

# =============================================================================
# Universal Kubernetes/Helm Deployment (Works with Kind & EKS)
# =============================================================================

## helm-deploy: Deploy or upgrade Arda Credit (auto-detects Kind/EKS)
.PHONY: helm-deploy
helm-deploy:
	@chmod +x ./script/helm-deploy.sh
	@./script/helm-deploy.sh deploy

## helm-status: Check deployment status
.PHONY: helm-status
helm-status:
	@chmod +x ./script/helm-deploy.sh
	@./script/helm-deploy.sh status

## helm-clean: Clean up deployment
.PHONY: helm-clean
helm-clean:
	@chmod +x ./script/helm-deploy.sh
	@./script/helm-deploy.sh clean

## helm-help: Show deployment commands
.PHONY: helm-help
helm-help:
	@echo "🎯 Universal Deployment Commands (Kind & EKS):"
	@echo ""
	@echo "  make helm-deploy   - Deploy/upgrade application (auto-detects environment)"
	@echo "  make helm-status   - Check deployment status"
	@echo "  make helm-clean    - Clean up deployment"
	@echo ""
	@echo "📝 Requirements:"
	@echo "  - .env file with secrets (SINDRI_API_KEY, ETHEREUM_*, SIGNOZ_INGESTION_KEY, etc.)"
	@echo "  - kubectl context set to target cluster"
	@echo "  - For Kind: GHCR_USERNAME/GHCR_TOKEN in .env"
	@echo "  - For EKS: Proper AWS credentials and cluster access"
