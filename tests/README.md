# Tests Directory

This directory contains all test files for the Arda Credit project.

## Structure

```
tests/
├── integration/          # Rust integration tests
│   ├── document_integration_tests.rs
│   ├── test_financial_api.rs
│   ├── test_marketplace_schema.rs
│   └── test_remove_from_sale.rs
│
└── scripts/             # Shell script tests
    ├── test_cli_trading_workflow.sh
    ├── test_loan_trading_integration.sh
    ├── test_local_anvil_setup.sh
    ├── test_metrics.sh
    ├── test_trading_error_scenarios.sh
    ├── test_trading_performance.sh
    ├── test_user_types_db_only.sh
    ├── test_user_types_integration.sh
    └── validate_metrics_fixes.sh
```

## Running Tests

### Rust Integration Tests

All Rust integration tests are automatically discovered by Cargo:

```bash
# Run all integration tests
cargo test --tests

# Run a specific integration test
cargo test --test test_remove_from_sale
cargo test --test test_marketplace_schema
cargo test --test test_financial_api
cargo test --test document_integration_tests
```

### Shell Script Tests

Shell scripts can be run directly:

```bash
# Make sure database is running
make up

# Run a specific test script
./tests/scripts/test_local_anvil_setup.sh
./tests/scripts/test_user_types_db_only.sh
./tests/scripts/test_metrics.sh

# Or with bash
bash tests/scripts/test_cli_trading_workflow.sh
```

## Test Categories

### Integration Tests (`integration/`)

These are Rust-based integration tests that test the full stack:
- **document_integration_tests.rs** - Document service integration tests
- **test_financial_api.rs** - Financial transaction API tests
- **test_marketplace_schema.rs** - Deal marketplace schema validation
- **test_remove_from_sale.rs** - Deal listing removal functionality

### Script Tests (`scripts/`)

Shell scripts for end-to-end testing and validation:
- **test_cli_trading_workflow.sh** - CLI-based trading workflow
- **test_loan_trading_integration.sh** - Full deal trading integration test
- **test_local_anvil_setup.sh** - Local Anvil devnet setup validation
- **test_metrics.sh** - Metrics system validation
- **test_trading_*.sh** - Various trading scenario tests
- **test_user_types_*.sh** - User type and permissions tests
- **validate_metrics_fixes.sh** - Metrics system fix validation

## Prerequisites

Most tests require:
1. PostgreSQL database running (`make up`)
2. Environment variables configured (`.env` file)
3. For blockchain tests: Anvil devnet running (`make anvil-start`)

See the main project README for setup instructions.
