# i2p - Universal Dynamic Prompt Generator

**i2p** (Issue-to-Prompt) is an AI-powered system that transforms any codebase issue into AI-agent-ready implementation specifications.

## Overview

The i2p system uses **dynamic codebase discovery** with **AST-powered file selection** to generate implementation prompts optimized for AI-to-AI handoff (Cursor subagents, Claude Code, etc.).

### Key Features

- 🎯 **Universal Codebase Analysis** - Works with any programming language/framework
- 🧠 **Multi-LLM Intelligence** - Claude Sonnet-4 for precision, Nemotron Nano for bulk processing, Grok-4 for evaluation
- 🔍 **AST-Powered Discovery** - Uses ast-grep for intelligent file selection
- 📊 **Cost Optimization** - Smart model selection based on complexity and context size
- 🤖 **AI-to-AI Optimized** - Generates atomic implementation units with exact line numbers
- ✅ **Quality Assurance** - Built-in evaluation layer for objective assessment

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

2. **Usage**: Run from project root:
   ```bash
   make i2p ISSUE="Add comprehensive observability metrics to batch processor"
   make i2p ISSUE="Optimize database connection pooling for high throughput"  
   make i2p ISSUE="Implement rate limiting for API endpoints"
   ```

## System Architecture

### Layer 1: Dynamic Tree Indexer
- **File**: `DynamicTreeIndexer` class
- **Purpose**: Generates comprehensive codebase structure index
- **Output**: Language detection, entry points, directory purposes, framework detection

### Layer 2: Universal AST Pattern Generator  
- **File**: `UniversalASTPatternGenerator` class
- **Purpose**: Creates AST-grep queries based on issue analysis
- **Features**: Language-specific patterns, cross-language queries, relevance scoring

### Layer 3: AST Query Executor & Results Analyzer
- **File**: `ASTQueryExecutor` + `IntelligentFileSelector` classes  
- **Purpose**: Executes AST queries and selects most relevant files
- **Output**: Scored file selection with diversity optimization

### Layer 4: Dynamic I2P Engine
- **File**: `DynamicI2P` class (main orchestrator)
- **Purpose**: Coordinates all layers and generates final AI-optimized prompts
- **Features**: Domain detection, cost tracking, quality evaluation

## Multi-LLM Architecture

### Model Selection Strategy

| Model | Use Case | Cost (per 1M tokens) | Selection Criteria |
|-------|----------|---------------------|-------------------|
| **Nemotron Nano** | Bulk processing, pattern analysis | Input: $0.04, Output: $0.16 | Code size > 50K chars |
| **Claude Sonnet-4** | Precision analysis, complex reasoning | Input: $3.00, Output: $15.00 | Complex issues, high accuracy needed |
| **Grok-4** | Objective evaluation, quality assessment | Input: $5.00, Output: $15.00 | Pure AI-to-AI evaluation |

### Domain-Specific Intelligence

The system includes specialized templates for:

- **zk_observability** - Zero-knowledge proof pipeline optimization
- **database_optimization** - Connection pooling, query performance  
- **api_security** - Authentication, rate limiting, security headers
- **error_handling** - Structured errors, logging, recovery strategies

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
├── README.md              # This documentation
├── requirements.txt       # Python dependencies
├── i2p.py                # Main i2p system
├── i2p_dynamic_prompt.md  # Generated output (latest)
└── .env                  # Local environment (not committed)
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

### Writing Effective Issues

**Good Examples:**
- "Add comprehensive observability metrics to batch processor with Sindri cost tracking"
- "Optimize database connection pooling for high-throughput transaction processing"
- "Implement JWT-based authentication with rate limiting for API endpoints"

**Avoid:**
- Vague requests: "Fix the code"
- Too broad: "Improve performance" 
- Too narrow: "Change line 42"

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

## Contributing

When contributing to i2p system:

1. Maintain existing code patterns and conventions
2. Add comprehensive docstrings for new methods
3. Update domain templates for new use cases  
4. Test with various issue types and complexity levels
5. Ensure cost efficiency in LLM usage

## License

Part of the Arda Credit project. See main project LICENSE for details.