# Arda Credit CLI - File Structure and Standards

This document outlines the standardized file structure and coding conventions for the Arda Credit CLI package.

## 📁 File Structure Overview

### Core Infrastructure (Fixed Structure)
```
cli/src/
├── lib.rs                           # Main library exports and module declarations
├── cli.rs                           # Core CLI setup and middleware configuration
└── bin/
    └── cli.rs                       # Main CLI entry point
```

### Module Organization Examples

**Single-File Modules** (when under 1000 lines):
```
├── balance.rs                       # Balance and wallet operations
├── batch.rs                         # Batch processing and proof operations
├── pools.rs                         # Liquidity pool operations
├── user.rs                          # User management and KYC operations
```

**Directory Modules** (when functionality requires multiple files):
```
├── loans/                           # Credit and loan operations
│   ├── mod.rs                       # Main module with command routing
│   ├── investors.rs                 # Investor registration and profiles
│   ├── merchants.rs                 # Merchant registration and turnover
│   ├── credit.rs                    # Credit line eligibility and requests
│   └── operations.rs                # Loan origination, payments, closeouts
```

**Feature-Gated Modules** (optional functionality):
```
├── benchmarks/                      # Performance benchmarks (requires --features testing)
│   ├── mod.rs                       # Main benchmark module
│   ├── analytics.rs                 # Analytics and statistics
│   ├── performance.rs               # Performance testing
│   ├── stress.rs                    # Stress testing
│   └── verification.rs              # Proof verification
├── tests/                           # Integration tests
│   └── integration_tests.rs         # End-to-end workflow tests
```

## 🎯 File Organization Standards

### 1. Single Responsibility Principle

- Each file handles one clear, focused responsibility
- Files **must not exceed 1000 lines of code**
- Large files must be split into logical, focused modules

### 2. Core Infrastructure Files

- **`lib.rs`** - Main exports, module declarations, and common utilities
- **`cli.rs`** - CLI setup, argument parsing, command routing, and middleware configuration

### 3. Command Structure

Each module exports:
- **Command enum** (`ModuleCommands`) defining all available operations
- **Handler function** (`handle_module_command`) for command routing
- **Individual operation functions** for specific business logic

### 4. Module Splitting Pattern

When a module grows beyond 1000 lines, split it into a directory structure. The `loans` module demonstrates this pattern:

**Before**: `loans.rs` (1,585 lines)
**After**: `loans/` directory with focused sub-modules:

```rust
// loans/mod.rs - Main module with command enum and routing
pub mod investors;
pub mod merchants; 
pub mod credit;
pub mod operations;

pub use investors::*;
pub use merchants::*;
pub use credit::*;
pub use operations::*;

#[derive(Subcommand)]
pub enum LoanCommands {
    // All loan-related commands
}

pub async fn handle_loan_command(/* ... */) -> Result<()> {
    // Route to appropriate sub-module
}
```

**Sub-modules by responsibility**:
- `investors.rs` (343 lines) - Investor registration and profiles
- `merchants.rs` (287 lines) - Merchant registration and turnover data
- `credit.rs` (283 lines) - Credit line eligibility and requests  
- `operations.rs` (286 lines) - Loan origination, payments, closeouts

## 📝 Coding Standards

### Command Documentation

All commands must have comprehensive documentation:

```rust
/// Create a new user account
Create {
    /// User type (user, investor, originator, admin)
    #[arg(long)]
    user_type: String,
    /// User email address
    #[arg(long)]
    email: String,
    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
},
```

### Function Documentation

All public functions must have doc comments explaining:
- Purpose and functionality
- Parameters and their validation
- Return values and error conditions
- Privacy implications (for ZK operations)

```rust
/// Create a new user account with KYC initialization
/// 
/// This function creates a user account and initializes their KYC status.
/// All user data is handled with privacy preservation in mind.
///
/// # Arguments
/// * `client` - API client for server communication
/// * `user_type` - Type of user (investor, lender, etc.)
/// * `email` - User's email address (validated)
/// * `verbose` - Enable detailed output
pub async fn create_user(
    client: &BatchApiClient,
    user_type: &str,
    email: &str,
    verbose: bool,
) -> Result<()> {
    // Implementation with comprehensive error handling
}
```

### Error Handling Standards

Consistent error handling across all modules:

```rust
// ✅ GOOD - Consistent error responses with user-friendly messages
match operation().await {
    Ok(result) => {
        println!("✅ Operation successful!");
        println!("   Details: {}", result);
    },
    Err(e) => {
        eprintln!("❌ Operation failed: {}", e);
        eprintln!("💡 Try: Alternative approach or troubleshooting steps");
    }
}
```

### Output Standards

Use consistent emoji and formatting for CLI output:

```rust
// Status indicators
println!("✅ Success message");
println!("⚠️  Warning message"); 
println!("❌ Error message");
println!("🔄 Processing message");
println!("💡 Helpful tip");

// Section headers
println!("👤 User Operations");
println!("💰 Balance Information"); 
println!("🏦 Pool Details");
println!("🔧 Technical Details (verbose)");
```

## 🧪 Testing Structure

### Unit Tests
Each module should include comprehensive unit tests following Rust best practices:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_function_success() {
        // Test implementation
    }
    
    #[tokio::test] 
    async fn test_function_error_cases() {
        // Test error handling
    }
}
```

### Integration Tests
- Place integration tests in `tests/` directory at the CLI package root
- Test complete command workflows end-to-end
- Validate CLI argument parsing and error handling

## 🔄 Backward Compatibility

Legacy commands are maintained through deprecated command support with clear migration guidance.

## 🤝 Contribution Guidelines

### Adding New Commands

1. **Identify the correct module** - Place commands in the appropriate functional module
2. **Follow naming conventions** - Use consistent command and function naming
3. **Add documentation** - Include comprehensive doc comments
4. **Implement error handling** - Use consistent error patterns
5. **Add tests** - Include unit tests for new functionality
6. **Update README** - Document new commands in this file

### Code Review Checklist

- [ ] Function is in the correct module (single responsibility)
- [ ] Function does not exceed reasonable length (~100 lines)
- [ ] All parameters are validated appropriately
- [ ] Error handling follows established patterns
- [ ] Output formatting uses consistent emoji and structure
- [ ] Documentation is comprehensive and helpful
- [ ] Tests cover success and error cases

## 🔍 Future Enhancements

The modular structure enables:

1. **Independent development** - Teams can work on different modules simultaneously
2. **Selective testing** - Test only affected modules during development
3. **Feature flags** - Enable/disable entire functional areas
4. **Plugin architecture** - Add new modules without affecting existing code
5. **Performance optimization** - Optimize specific modules based on usage patterns