# Arda Credit CLI - File Structure and Standards

This document outlines the standardized file structure and coding conventions for the Arda Credit CLI package.

## 📁 File Structure Overview

```
cli/src/
├── lib.rs                           # Main library exports and module declarations
├── cli.rs                           # Core CLI setup and middleware configuration
├── balance.rs                       # Balance and wallet operations
├── batch.rs                         # Batch processing and proof operations
├── loans.rs                         # Credit and loan operations
├── pools.rs                         # Liquidity pool operations
├── testing.rs                       # Testing and performance operations
├── user.rs                          # User management and KYC operations
└── bin/
    └── cli.rs                       # Main CLI entry point
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

Example structure:
```rust
#[derive(Subcommand)]
pub enum UserCommands {
    Create { /* fields */ },
    Get { /* fields */ },
    // ... other commands
}

pub async fn handle_user_command(
    client: &BatchApiClient,
    command: UserCommands,
    verbose: bool,
) -> Result<()> { /* routing logic */ }

pub async fn create_user(/* params */) -> Result<()> { /* implementation */ }
```

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