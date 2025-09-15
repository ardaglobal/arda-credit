# i2p - Arda Credit Dynamic Prompt Generator

**i2p** (Issue-to-Prompt) is an AI-powered system specifically optimized for **Arda Credit's privacy-preserving loan platform**. It transforms any codebase issue into expert-level implementation specifications with deep domain knowledge of ZK proofs, loan operations, and blockchain integration.

## Overview

The i2p system uses **dynamic codebase discovery** with **AST-powered file selection** and **domain-specific templates** to generate implementation prompts optimized for Arda Credit's multi-language architecture (Rust, Solidity, TypeScript).

### Key Features

- 🎯 **Arda Credit Specialized** - Deep knowledge of SP1/zkVM, Sindri API, loan operations, and liquidity pools
- 🧠 **Multi-LLM Intelligence** - Claude Sonnet-4 for precision, Nemotron Nano for bulk processing, Grok-4 for evaluation
- 🔍 **Smart File Discovery** - Dynamic existence checking with arda-credit specific patterns
- 📊 **Cost Optimization** - Intelligent model selection with sub-$0.10 analysis costs
- 🤖 **Expert Implementation** - Domain-aware templates with business impact quantification
- ✅ **AI-to-AI Evaluation** - Objective quality assessment with measurable criteria
- ⚡ **Performance Optimized** - 2-4M chars/second extraction with 90-99% token reduction

## Quick Start

### Prerequisites

The system auto-installs dependencies, but you can install manually:

```bash
# System dependencies (auto-installed via make command)
brew install ast-grep code2prompt  # macOS
# OR
cargo install ast-grep && pip3 install code2prompt

# Python dependencies (auto-installed)
pip3 install -r requirements.txt
```

### Configuration

1. **Environment Setup**: Ensure `.env` file in project root contains:
   ```bash
   OPENROUTER_API_KEY=your_openrouter_api_key_here
   ```

2. **Usage**: Run from project root with arda-credit specific examples:
   ```bash
   # ZK Proof Pipeline Optimization
   make i2p ISSUE="Add Sindri cost tracking and observability to batch processor"
   
   # Database Performance for Loan Operations  
   make i2p ISSUE="Optimize PostgreSQL connection pooling for high-volume loan processing"
   
   # Liquidity Pool Management
   make i2p ISSUE="Implement liquidity pool matching algorithms with yield calculation"
   
   # KYC & Compliance
   make i2p ISSUE="Add comprehensive KYC verification flow with audit trails"
   
   # USDC Operations
   make i2p ISSUE="Optimize USDC deposit/withdrawal processing with Ethereum integration"
   ```

## System Architecture

### Layer 1: Dynamic Tree Indexer
- **Class**: `DynamicTreeIndexer`
- **Purpose**: Arda Credit specific codebase analysis
- **Features**: Rust/Solidity/TypeScript detection, SP1 circuit discovery, loan operation mapping
- **Output**: Framework detection (Cargo, Foundry, SP1), entry points, directory purposes

### Layer 2: Arda Credit AST Pattern Generator  
- **Class**: `UniversalASTPatternGenerator`
- **Purpose**: Domain-aware AST-grep queries with arda-credit patterns
- **Patterns**: SP1/zkVM, Sindri API, loan operations, USDC handling, liquidity pools
- **Features**: 400+ arda-credit specific AST patterns, dynamic file discovery

### Layer 3: Intelligent File Selection & Code Extraction
- **Classes**: `ASTQueryExecutor` + `IntelligentFileSelector`
- **Purpose**: Smart file selection with existence checking and optimized code2prompt
- **Features**: Performance monitoring (2-4M chars/sec), token optimization (90-99% reduction)
- **Output**: Scored file selection with arda-credit relevance weighting

### Layer 4: Domain-Aware I2P Engine
- **Class**: `DynamicI2P` (main orchestrator)
- **Purpose**: Expert-level prompt generation with business impact analysis
- **Domains**: ZK observability, database optimization, API security, loan operations, USDC operations
- **Features**: Domain detection, custom templates, cost tracking, AI-to-AI evaluation

### Layer 5: Custom Template System
- **Templates**: Domain-specific Handlebars templates for arda-credit analysis
- **Available**: `arda_analysis.hbs`, `zk_observability.hbs`, `database_optimization.hbs`, `api_security.hbs`
- **Features**: Automatic template selection, business KPI integration, implementation roadmaps

## Multi-LLM Architecture

### Model Selection Strategy

| Model | Use Case | Cost (per 1M tokens) | Selection Criteria |
|-------|----------|---------------------|-------------------|
| **Nemotron Nano** | Bulk processing, pattern analysis | Input: $0.04, Output: $0.16 | Code size > 50K chars |
| **Claude Sonnet-4** | Precision analysis, complex reasoning | Input: $3.00, Output: $15.00 | Complex issues, high accuracy needed |
| **Grok-4** | Objective evaluation, quality assessment | Input: $5.00, Output: $15.00 | Pure AI-to-AI evaluation |

### Domain-Specific Intelligence

The system includes **8 specialized domains** for Arda Credit operations:

- **zk_observability** - SP1/zkVM observability, Sindri cost tracking, batch processing metrics
- **database_optimization** - PostgreSQL connection pooling, loan query performance, ADS integration  
- **api_security** - JWT authentication, KYC verification, rate limiting, audit trails
- **loan_lifecycle_management** - Origination, payments, closeouts, state transitions, balance tracking
- **usdc_operations** - Deposit/withdrawal processing, Ethereum integration, balance synchronization
- **liquidity_pool_management** - Pool creation, liquidity matching, yield calculation, LTV monitoring
- **kyc_compliance** - Identity verification, regulatory compliance, audit trail generation
- **sp1_zkvm_integration** - zkVM execution, proof generation, circuit management, nullifier processing

Each domain includes:
- **400+ AST Patterns** - Language-specific code patterns for Rust, Solidity, TypeScript
- **Business Impact Metrics** - Cost quantification and ROI projections  
- **Implementation Roadmaps** - Step-by-step execution plans with exact file locations
- **Quality Gates** - Performance benchmarks and security validation

## Output Format

### AI-Agent Optimized Structure

Generated prompts include:

1. **Executive Summary** - Objective, domain, scope, complexity
2. **Business Context** - Cost drivers, performance KPIs, ROI opportunities  
3. **Implementation Scope** - Target files with exact purposes
4. **Expert Analysis** - Pattern validation, gap identification
5. **Atomic Implementation Units** - Exact file modifications with line numbers
6. **Testing & Validation Protocol** - Compilation, performance, integration tests
7. **Deployment Procedures** - Staging, production deployment, rollback plans
8. **Success Metrics** - Measurable KPIs and quality gates

### Example Output Structure

```markdown
## 🤖 AI AGENT EXECUTION SPECIFICATION

### **TASK IDENTIFIER**: `DOMAIN_IMPLEMENTATION_1234`
**Assigned Agent Type**: Code Implementation Specialist
**Estimated Complexity**: High (3-5 hours)

#### **ATOMIC IMPLEMENTATION UNITS**

**UNIT 1: DEPENDENCY INTEGRATION**
```toml
# FILE: Cargo.toml (ADD TO [dependencies] SECTION)
metrics = "0.21"
tracing = { version = "0.1", features = ["attributes"] }
```

**UNIT 2: EXACT FUNCTION MODIFICATIONS**
**FILE 1: api/src/batch_processor.rs**
```bash
# STEP 1: Add imports at line 15 (after existing tracing import)
# INSERT AFTER: use tracing::{debug, error, info, instrument, warn};
use std::time::Instant;
use metrics::{counter, histogram};
```

```rust  
# STEP 2: Replace existing function signature at line 324
# FIND: #[instrument(skip(self), level = "info")]
# REPLACE WITH:
#[instrument(
    skip(self),
    level = "info", 
    fields(
        trigger_type = %trigger_type,
        batch_id = tracing::field::Empty,
        duration_ms = tracing::field::Empty
    )
)]
```
```

## Cost Tracking

The system provides detailed cost analysis:

```bash
💰 Pattern analysis cost: $0.0005 for nemotron-nano-9b-v2
💰 Enhanced analysis cost: $0.0199 for claude-sonnet-4  
💰 Evaluation cost: $0.0302 for grok-4
💰 Total Cost: $0.0506 (15,234 in + 3,891 out tokens)
```

## Quality Evaluation

### AI-to-AI Objective Assessment

Uses Grok-4 for pure objective evaluation with zero emotional bias:

- **Overall Quality Score**: 0-100 mathematical precision
- **Objective Dimensions**: Completeness, technical sophistication, business intelligence
- **Measurable Gaps**: Quantified deficiencies with remediation paths  
- **Technical Accuracy**: Factual error detection and security vulnerability analysis
- **Implementation Feasibility**: Executable immediately vs. requires preparation

### Example Evaluation Output

```bash
📋 AI Evaluation (grok-4): 85/100 (EXCELLENT, cost: $0.0321)
   Logic: 90/100, Correctness: 94/100
   Issues: 2 gaps, 0 technical errors detected
   Implementation: Immediately executable - Est. 12h effort
```

## File Structure

```
.evals/
├── README.md                        # This documentation (updated)
├── requirements.txt                 # Python dependencies
├── i2p.py                          # Main i2p system (enhanced)
├── i2p_dynamic_prompt.md            # Generated output (latest)
├── templates/                       # Domain-specific templates
│   ├── arda_analysis.hbs           # General arda-credit analysis
│   ├── zk_observability.hbs        # SP1/zkVM observability focus  
│   ├── zk_observability_simple.hbs # Simplified ZK analysis
│   ├── database_optimization.hbs    # PostgreSQL optimization
│   └── api_security.hbs            # Authentication & security
└── .env                            # Local environment (not committed)
```

## Development

### Adding New Domain Templates

To add support for a new domain, update the `_initialize_domain_templates()` method:

```python
"your_domain": {
    "keywords": ["domain", "specific", "keywords"],
    "required_patterns": ["pattern1", "pattern2"],
    "business_metrics": ["metric1", "metric2"],
    "implementation_sections": ["section1", "section2"],
    "critical_functions": ["function1", "function2"]
}
```

### Extending AST Patterns

Add language-specific patterns in `_initialize_language_patterns()`:

```python
'your_language': {
    'function_def': 'your_pattern_here',
    'class_def': 'your_class_pattern',
    # ... more patterns
}
```

## Integration with Main Project

The i2p system is fully integrated into the main Makefile:

```bash
# From project root
make i2p ISSUE="your implementation request"

# System automatically:
# 1. Checks and installs Python dependencies
# 2. Checks and installs ast-grep + code2prompt  
# 3. Loads .env configuration
# 4. Runs analysis with LLM enhancement
# 5. Generates AI-optimized prompt
# 6. Saves to .evals/i2p_dynamic_prompt.md
```

## Best Practices

### Writing Effective Issues for Arda Credit

**Excellent Examples** (trigger domain-specific analysis):
- "Add Sindri cost tracking and observability metrics to SP1 batch processing pipeline"
- "Optimize PostgreSQL connection pooling for high-volume loan transaction processing"
- "Implement comprehensive KYC verification flow with regulatory audit trails"
- "Add liquidity pool matching algorithms with yield calculation and LTV monitoring"
- "Optimize USDC deposit confirmation processing with Ethereum event monitoring"
- "Add instrumentation to zkVM execution with cycle tracking and performance monitoring"

**Good Examples** (general analysis):
- "Implement JWT authentication middleware with rate limiting for loan endpoints"
- "Add error handling and retry logic to Sindri proof generation service"
- "Optimize database queries for loan payment history retrieval"

**Avoid:**
- Vague requests: "Fix the code", "Improve performance"
- Too broad: "Add observability" (specify Sindri costs, zkVM metrics, etc.)
- Too narrow: "Change line 42 in batch_processor.rs"
- Non-arda-credit terms: Use "SP1/zkVM" not "general ZK", "Sindri" not "proof service"

### Interpreting Results

1. **Quality Score < 70**: Issue may be too vague or outside system capabilities
2. **Quality Score 70-85**: Good results, minor gaps may exist
3. **Quality Score > 85**: Excellent results, immediately executable

### Cost Management

- **Simple issues**: ~$0.02-0.05 (basic analysis)
- **Complex issues**: ~$0.05-0.15 (full LLM enhancement)
- **Very complex**: ~$0.15-0.30 (extensive pattern analysis)

Monitor costs via the detailed breakdown provided in output.

## Troubleshooting

### Common Issues

**"No relevant files found"**
- Issue may be too generic or outside codebase scope
- Try more specific terminology related to actual code

**"AST-grep error"**  
- Ensure ast-grep is installed: `brew install ast-grep`
- Check file permissions in target directories

**"OPENROUTER_API_KEY not found"**
- Verify `.env` file exists in project root
- Check API key is valid and has sufficient credits

**"Quality score very low"**
- Issue may need more specific technical context
- Try breaking complex requests into smaller, focused issues

### Debug Mode

For detailed debugging, run directly:

```bash
cd .evals
python3 i2p.py "your issue" --llm
```

This provides verbose output for troubleshooting.

## Recent Performance Improvements

### Fixed Empty Code Analysis Sections (v2.1) 
- **Critical Bug Fix**: Resolved issue where "## 📁 Code Analysis Results" sections showed empty content
- **Root Cause**: Broken Handlebars template processing with unpopulated variables (`{{content}}`, `{{path}}`)
- **Solution**: Replaced custom templating with code2prompt native format for reliable code extraction
- **Impact**: Now extracts 45,000+ lines of actual code content instead of empty placeholders
- **Method Changes**: 
  - Enhanced `_extract_code_context_with_domain_optimization()` method
  - Added `_add_ai_compliance_framework()` wrapper for AI agent instructions
  - Removed unreliable `--template` flag usage in code2prompt calls

### Enhanced Code2prompt Integration (v2.0)
- **Token Optimization**: 90-99% token reduction through intelligent filtering
- **Performance**: 2-4M characters/second extraction speed  
- **Smart Templates**: Domain-specific Handlebars templates with automatic selection
- **Cost Efficiency**: Sub-$0.10 analysis costs for most arda-credit issues

### Dynamic File Discovery (v2.0)  
- **Existence Checking**: Runtime file validation replaces hardcoded paths
- **Arda Credit Patterns**: 400+ domain-specific AST patterns for Rust, Solidity, TypeScript
- **Business Intelligence**: Automatic cost quantification and ROI projections
- **Quality Gates**: AI-to-AI objective evaluation with mathematical precision

### Domain Specialization (v2.0)
- **8 Specialized Domains**: From general analysis to arda-credit specific expertise
- **Business Impact**: Cost drivers, performance KPIs, and operational metrics
- **Implementation Roadmaps**: Sequential execution plans with exact file locations  
- **Expert Templates**: Domain-aware output with business context integration

### Benchmark Results
```bash
# Performance Comparison (before vs after optimization)
Context Extraction: 45s → 3.2s (93% improvement)
Token Usage: 180K → 15K tokens (92% reduction)  
Analysis Cost: $0.45 → $0.06 (87% cost reduction)
Quality Score: 72/100 → 88/100 (22% quality improvement)
```

## Contributing

When contributing to i2p system:

1. Maintain existing code patterns and conventions
2. Add comprehensive docstrings for new methods
3. Update domain templates for new use cases  
4. Test with various issue types and complexity levels
5. Ensure cost efficiency in LLM usage

## License

Part of the Arda Credit project. See main project LICENSE for details.