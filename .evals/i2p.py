#!/usr/bin/env python3
"""
i2p Universal Dynamic Architecture
Dynamic codebase discovery with AST-powered file selection
"""

import os
import json
import threading
import time
import subprocess
from pathlib import Path
from dotenv import load_dotenv
import openai
from collections import Counter
from typing import Dict, List, Optional, Tuple

load_dotenv()

# OpenRouter client with correct model names
openrouter_client = openai.OpenAI(
    base_url="https://openrouter.ai/api/v1", 
    api_key=os.getenv('OPENROUTER_API_KEY')
)

# ============================================================================
# LAYER 1: DYNAMIC TREE INDEXER
# ============================================================================

class DynamicTreeIndexer:
    """Generates comprehensive codebase structure index"""
    
    def __init__(self, root_path: str):
        self.root_path = Path(root_path)
        self.ignore_patterns = {
            'dirs': {'.git', 'node_modules', 'target', 'build', 'dist', '__pycache__', '.pytest_cache'},
            'files': {'.DS_Store', '*.pyc', '*.log', '*.tmp'}
        }
    
    def generate_codebase_index(self) -> Dict:
        """Generate complete codebase structural analysis"""
        print("🌳 Generating dynamic tree index...")
        
        return {
            "file_tree": self._build_file_tree(),
            "language_map": self._detect_languages(),
            "entry_points": self._find_entry_points(),
            "directory_purposes": self._analyze_directory_purposes(),
            "framework_detection": self._detect_frameworks(),
            "size_metrics": self._calculate_size_metrics()
        }
    
    def _build_file_tree(self) -> Dict:
        """Build intelligent file tree with metadata"""
        tree = {}
        
        for root, dirs, files in os.walk(self.root_path):
            # Filter ignored directories
            dirs[:] = [d for d in dirs if d not in self.ignore_patterns['dirs']]
            
            rel_root = os.path.relpath(root, self.root_path)
            tree[rel_root] = {
                "files": [],
                "subdirs": dirs.copy(),
                "file_count": 0,
                "total_size": 0
            }
            
            for file in files:
                if not self._should_ignore_file(file):
                    file_path = os.path.join(root, file)
                    file_info = self._analyze_file_metadata(file_path)
                    tree[rel_root]["files"].append(file_info)
                    tree[rel_root]["file_count"] += 1
                    tree[rel_root]["total_size"] += file_info.get("size", 0)
        
        return tree
    
    def _should_ignore_file(self, filename: str) -> bool:
        """Check if file should be ignored"""
        for pattern in self.ignore_patterns['files']:
            if pattern.startswith('*') and filename.endswith(pattern[1:]):
                return True
            elif filename == pattern:
                return True
        return False
    
    def _analyze_file_metadata(self, file_path: str) -> Dict:
        """Extract metadata from individual file"""
        try:
            stat = os.stat(file_path)
            return {
                "name": os.path.basename(file_path),
                "extension": Path(file_path).suffix,
                "size": stat.st_size,
                "modified": stat.st_mtime,
                "relative_path": os.path.relpath(file_path, self.root_path)
            }
        except OSError:
            return {"name": os.path.basename(file_path), "error": "Cannot access"}
    
    def _detect_languages(self) -> Dict:
        """Detect programming languages in codebase"""
        language_stats = Counter()
        
        for root, dirs, files in os.walk(self.root_path):
            dirs[:] = [d for d in dirs if d not in self.ignore_patterns['dirs']]
            
            for file in files:
                if not self._should_ignore_file(file):
                    ext = Path(file).suffix.lower()
                    language = self._extension_to_language(ext)
                    if language:
                        language_stats[language] += 1
        
        return dict(language_stats)
    
    def _extension_to_language(self, ext: str) -> Optional[str]:
        """Map file extension to programming language"""
        mapping = {
            '.rs': 'rust',
            '.py': 'python', 
            '.js': 'javascript',
            '.ts': 'typescript',
            '.sol': 'solidity',
            '.go': 'go',
            '.java': 'java',
            '.cpp': 'cpp',
            '.c': 'c',
            '.sh': 'shell',
            '.yml': 'yaml',
            '.yaml': 'yaml',
            '.toml': 'toml',
            '.json': 'json'
        }
        return mapping.get(ext)
    
    def _find_entry_points(self) -> List[str]:
        """Identify main entry point files"""
        entry_patterns = [
            'main.rs', 'lib.rs', 'mod.rs',
            'main.py', '__init__.py',
            'index.js', 'app.js', 'server.js',
            'main.go',
            'Main.java'
        ]
        
        entry_points = []
        for root, dirs, files in os.walk(self.root_path):
            dirs[:] = [d for d in dirs if d not in self.ignore_patterns['dirs']]
            
            for file in files:
                if file in entry_patterns:
                    rel_path = os.path.relpath(os.path.join(root, file), self.root_path)
                    entry_points.append(rel_path)
        
        return entry_points
    
    def _analyze_directory_purposes(self) -> Dict[str, str]:
        """Infer directory purposes from naming conventions"""
        purpose_patterns = {
            'api': ['api', 'server', 'backend', 'handlers'],
            'database': ['db', 'database', 'models', 'migrations'],
            'frontend': ['frontend', 'ui', 'web', 'client'],
            'cli': ['cli', 'cmd', 'command'],
            'tests': ['test', 'tests', 'spec'],
            'docs': ['doc', 'docs', 'documentation'],
            'config': ['config', 'configuration', 'settings'],
            'utils': ['util', 'utils', 'helpers', 'lib'],
            'contracts': ['contract', 'contracts', 'solidity'],
            'scripts': ['script', 'scripts', 'tools']
        }
        
        directory_purposes = {}
        
        for root, dirs, files in os.walk(self.root_path):
            for dir_name in dirs:
                if dir_name not in self.ignore_patterns['dirs']:
                    dir_lower = dir_name.lower()
                    for purpose, patterns in purpose_patterns.items():
                        if any(pattern in dir_lower for pattern in patterns):
                            rel_path = os.path.relpath(os.path.join(root, dir_name), self.root_path)
                            directory_purposes[rel_path] = purpose
                            break
        
        return directory_purposes
    
    def _detect_frameworks(self) -> Dict[str, bool]:
        """Detect frameworks and technologies used"""
        frameworks = {
            'rust_cargo': False,
            'node_js': False,
            'python_poetry': False,
            'go_modules': False,
            'foundry': False,
            'docker': False
        }
        
        # Check for framework indicator files
        indicators = {
            'Cargo.toml': 'rust_cargo',
            'package.json': 'node_js', 
            'pyproject.toml': 'python_poetry',
            'go.mod': 'go_modules',
            'foundry.toml': 'foundry',
            'Dockerfile': 'docker'
        }
        
        for indicator_file, framework in indicators.items():
            if (self.root_path / indicator_file).exists():
                frameworks[framework] = True
        
        return frameworks
    
    def _calculate_size_metrics(self) -> Dict:
        """Calculate codebase size statistics"""
        total_files = 0
        total_size = 0
        language_sizes = Counter()
        
        for root, dirs, files in os.walk(self.root_path):
            dirs[:] = [d for d in dirs if d not in self.ignore_patterns['dirs']]
            
            for file in files:
                if not self._should_ignore_file(file):
                    file_path = os.path.join(root, file)
                    try:
                        size = os.path.getsize(file_path)
                        total_files += 1
                        total_size += size
                        
                        ext = Path(file).suffix.lower()
                        language = self._extension_to_language(ext)
                        if language:
                            language_sizes[language] += size
                    except OSError:
                        continue
        
        return {
            "total_files": total_files,
            "total_size_bytes": total_size,
            "size_by_language": dict(language_sizes)
        }

# ============================================================================
# LAYER 2: UNIVERSAL AST PATTERN GENERATOR
# ============================================================================

class UniversalASTPatternGenerator:
    """Generates AST-grep queries for any issue type across all languages"""
    
    def __init__(self, codebase_index: Dict):
        self.codebase_index = codebase_index
        self.language_patterns = self._initialize_language_patterns()
    
    def generate_ast_queries(self, issue: str) -> List[str]:
        """Generate targeted AST queries based on issue and codebase analysis"""
        print(f"🔍 Generating AST queries for: {issue}")
        
        # Determine relevant languages and directories
        relevant_languages = self._identify_relevant_languages(issue)
        relevant_directories = self._identify_relevant_directories(issue)
        
        # Generate language-specific AST patterns
        ast_queries = []
        
        for language in relevant_languages:
            lang_queries = self._generate_language_specific_queries(issue, language, relevant_directories)
            ast_queries.extend(lang_queries)
        
        # Add cross-language patterns
        cross_lang_queries = self._generate_cross_language_queries(issue, relevant_directories)
        ast_queries.extend(cross_lang_queries)
        
        print(f"✅ Generated {len(ast_queries)} AST queries")
        return ast_queries
    
    def _initialize_language_patterns(self) -> Dict:
        """Initialize AST-grep compatible patterns for arda-credit architecture"""
        return {
            'rust': {
                # Core Rust patterns with proper ast-grep syntax
                'function_def': 'fn $NAME($$$PARAMS) -> $RETURN { $$$BODY }',
                'async_function': 'async fn $NAME($$$PARAMS) -> $RETURN { $$$BODY }',
                'struct_def': 'struct $NAME { $$$FIELDS }',
                'impl_block': 'impl $NAME { $$$METHODS }',
                'use_statement': 'use $MODULE::$ITEM;',
                'method_call': '$OBJ.$METHOD($$$ARGS)',
                'result_type': 'Result<$OK, $ERR>',
                'trait_def': 'trait $NAME { $$$METHODS }',
                'macro_call': '$MACRO!($$$ARGS)',
                
                # SP1/zkVM specific patterns for arda-credit
                'sp1_derive': '#[sp1_derive::AlignedBorrow]',
                'sp1_program': 'sp1_sdk::utils::setup_logger()',
                'proof_with_values': 'ProofWithPublicValues<$TYPE>',
                'sp1_public_values': 'SP1PublicValues',
                'zkvm_read': 'sp1_zkvm::io::read::<$TYPE>()',
                'zkvm_commit': 'sp1_zkvm::io::commit(&$VALUE)',
                'sp1_client_setup': 'SP1ProverClient::new()',
                'sp1_verifier_setup': 'SP1VerifyingKey::from_bytes($BYTES)',
                'sp1_prove_call': '$CLIENT.prove($PROGRAM, $STDIN)',
                'sp1_verify_call': '$CLIENT.verify(&$PROOF, &$VK)',
                'zkvm_execution_env': 'sp1_zkvm::utils::setup_logger()',
                'zkvm_program_entrypoint': 'sp1_zkvm::entrypoint!($FUNC)',
                'zkvm_io_operations': 'sp1_zkvm::io::$OPERATION::<$TYPE>($$$ARGS)',
                'sp1_stdin_write': '$STDIN.write(&$DATA)',
                'sp1_core_proof': 'SP1ProofMode::Core',
                'sp1_compressed_proof': 'SP1ProofMode::Compressed',
                'sp1_plonk_proof': 'SP1ProofMode::Plonk',
                'zkvm_cycle_tracking': 'sp1_zkvm::utils::cycle_tracker::start($LABEL)',
                'zkvm_cycle_end': 'sp1_zkvm::utils::cycle_tracker::end($LABEL)',
                
                # Database patterns specific to arda-credit
                'sqlx_pool': 'PgPoolOptions::new()',
                'sqlx_query': 'sqlx::query!($QUERY)',
                'database_transaction': 'pool.begin().await',
                'connection_pool': '$POOL.acquire().await',
                'max_connections': '$POOL.max_connections($SIZE)',
                
                # API/Server patterns for arda-credit
                'axum_router': 'Router::new()',
                'axum_handler': 'async fn $NAME($$$) -> $RESPONSE',
                'tracing_instrument': '#[instrument($$$ARGS)]',
                'api_endpoint': '.route("$PATH", $METHOD($HANDLER))',
                
                # ZK/Proof patterns specific to arda-credit
                'batch_processor': 'BatchProcessor::process_batch($$$)',
                'sindri_client': 'sindri_client.prove($$$ARGS)',
                'proof_generation': 'generate_batch_proof($$$)',
                'loan_transaction': 'LoanTransaction { transaction_type: $TYPE, $$$ }',
                'batch_creation': 'create_batch_with_ads($$$ARGS)',
                'proof_polling': 'poll_sindri_status($PROOF_ID)',
                'proof_submission': 'submit_batch_to_sindri($BATCH)',
                'nullifier_generation': 'transaction_to_nullifier($TRANSACTION)',
                'merkle_tree_update': 'imt.insert($NULLIFIER)',
                'state_root_transition': 'previous_state_root -> new_state_root',
                'ads_service_call': 'ads_service.$METHOD($$$ARGS)',
                'loan_processor_execute': 'LoanProcessor::process_transactions($TRANSACTIONS)',
                'balance_state_update': 'balance_state.apply_update($UPDATE)',
                'transaction_validation': 'validate_transaction_with_state($TRANSACTION, $STATE)',
                
                # Error handling patterns
                'thiserror': '#[derive(Error, Debug)]',
                'error_enum': 'enum $NAME { $$$VARIANTS }',
                'anyhow_result': 'anyhow::Result<$TYPE>',
                
                # Authentication patterns for arda-credit
                'jwt_decode': 'decode::<$CLAIMS>($TOKEN, $KEY, $VALIDATION)',
                'kyc_verification': 'verify_kyc_status($USER)',
                'auth_middleware': 'auth_middleware($$$)'
            },
            
            'solidity': {
                # Enhanced Solidity patterns for arda-credit contracts
                'contract_def': 'contract $NAME { $$$BODY }',
                'function_def': 'function $NAME($$$PARAMS) $VISIBILITY returns($RETURN) { $$$BODY }',
                'modifier_def': 'modifier $NAME($$$PARAMS) { $$$BODY }',
                'event_def': 'event $NAME($$$PARAMS);',
                'state_var': '$TYPE $VISIBILITY $NAME;',
                'constructor': 'constructor($$$PARAMS) { $$$BODY }',
                'interface_def': 'interface $NAME { $$$FUNCTIONS }',
                'library_def': 'library $NAME { $$$FUNCTIONS }',
                
                # SP1 verification patterns for arda-credit
                'sp1_verifier': 'ISP1Verifier($VERIFIER)',
                'proof_verification': 'verifyProof($PROOF, $PUBLIC_VALUES)',
                'loan_state_update': 'updateLoanState($$$PARAMS)',
                'sp1_verifier_interface': 'interface ISP1Verifier { $$$FUNCTIONS }',
                'sp1_proof_struct': 'struct SP1Proof { $$$FIELDS }',
                'sp1_public_values': 'struct SP1PublicValues { $$$FIELDS }',
                'proof_verification_call': '$VERIFIER.verifyProof($VK, $PUBLIC_VALUES, $PROOF)',
                'vkey_validation': 'require($VK != bytes32(0), "Invalid verification key")',
                'proof_validation': 'require($PROOF.length > 0, "Invalid proof")',
                
                # USDC/ERC20 patterns for arda-credit
                'erc20_transfer': 'IERC20($TOKEN).transfer($TO, $AMOUNT)',
                'usdc_deposit': 'usdcToken.transferFrom($FROM, address(this), $AMOUNT)',
                'balance_check': 'balanceOf($ACCOUNT)',
                'erc20_interface': 'interface IERC20 { $$$FUNCTIONS }',
                'usdc_token_setup': 'IERC20 public immutable usdcToken',
                'safe_transfer': 'SafeERC20.safeTransfer($TOKEN, $TO, $AMOUNT)',
                'safe_transfer_from': 'SafeERC20.safeTransferFrom($TOKEN, $FROM, $TO, $AMOUNT)',
                'token_approval': '$TOKEN.approve($SPENDER, $AMOUNT)',
                'allowance_check': '$TOKEN.allowance($OWNER, $SPENDER)',
                
                # Loan contract patterns
                'loan_origination': 'originateLoan($$$PARAMS)',
                'loan_payment': 'makeLoanPayment($$$PARAMS)',
                'loan_closeout': 'closeoutLoan($$$PARAMS)',
                'loan_state_enum': 'enum LoanState { $$$STATES }',
                'loan_struct': 'struct Loan { $$$FIELDS }',
                'loan_event': 'event Loan$ACTION($$$PARAMS)',
                'loan_modifier': 'modifier onlyValidLoan($LOAN_ID) { $$$BODY }',
                'loan_storage': 'mapping($KEY => $VALUE) public $NAME',
                
                # Access control patterns for arda-credit
                'ownable_modifier': 'modifier onlyOwner() { $$$BODY }',
                'role_based_access': 'modifier onlyRole($ROLE) { $$$BODY }',
                'access_control_import': 'import "@openzeppelin/contracts/access/AccessControl.sol"',
                'pausable_modifier': 'modifier whenNotPaused() { $$$BODY }',
                'reentrancy_guard': 'modifier nonReentrant() { $$$BODY }',
                
                # Error handling patterns for Solidity
                'custom_error': 'error $NAME($$$PARAMS)',
                'revert_with_error': 'revert $ERROR($$$ARGS)',
                'require_check': 'require($CONDITION, "$MESSAGE")',
                'assert_check': 'assert($CONDITION)',
                
                # State management patterns
                'state_variable_update': '$STATE_VAR = $NEW_VALUE',
                'mapping_update': '$MAPPING[$KEY] = $VALUE',
                'array_push': '$ARRAY.push($ELEMENT)',
                'struct_initialization': '$STRUCT_TYPE({$$$FIELDS})',
                
                # Liquidity pool patterns for arda-credit
                'pool_creation': 'createLiquidityPool($$$PARAMS)',
                'pool_deposit': 'depositToPool($POOL_ID, $AMOUNT)',
                'pool_withdrawal': 'withdrawFromPool($POOL_ID, $AMOUNT)',
                'pool_matching': 'findMatchingPools($CRITERIA)',
                'yield_calculation': 'calculateYield($POOL_ID, $PRINCIPAL, $TIME)',
                'ltv_validation': 'validateLTV($COLLATERAL, $LOAN_AMOUNT, $MAX_LTV)'
            },
            
            'javascript': {
                'function_def': 'function $NAME($$$PARAMS) { $$$BODY }',
                'arrow_function': '($$$PARAMS) => $BODY',
                'async_function': 'async function $NAME($$$PARAMS) { $$$BODY }',
                'class_def': 'class $NAME { $$$METHODS }',
                'import_statement': 'import $ITEM from "$MODULE"',
                'export_statement': 'export $ITEM',
                'method_call': '$OBJ.$METHOD($$$ARGS)',
                'await_call': 'await $EXPRESSION',
                'promise_all': 'Promise.all([$$$PROMISES])',
                'try_catch': 'try { $$$TRY } catch ($ERR) { $$$CATCH }'
            },
            
            'typescript': {
                'function_def': 'function $NAME($$$PARAMS): $RETURN { $$$BODY }',
                'async_function': 'async function $NAME($$$PARAMS): Promise<$RETURN> { $$$BODY }',
                'interface_def': 'interface $NAME { $$$PROPERTIES }',
                'type_alias': 'type $NAME = $TYPE',
                'class_def': 'class $NAME { $$$MEMBERS }',
                'import_statement': 'import { $ITEMS } from "$MODULE"',
                'export_type': 'export type $NAME = $TYPE',
                'generic_function': 'function $NAME<$GENERICS>($$$PARAMS): $RETURN { $$$BODY }'
            },
            
            'python': {
                'function_def': 'def $NAME($$$PARAMS): $$$BODY',
                'async_function': 'async def $NAME($$$PARAMS): $$$BODY',
                'class_def': 'class $NAME: $$$BODY',
                'import_statement': 'import $MODULE',
                'from_import': 'from $MODULE import $ITEMS',
                'method_call': '$OBJ.$METHOD($$$ARGS)',
                'try_except': 'try: $$$TRY except $EXCEPTION: $$$EXCEPT',
                'with_statement': 'with $CONTEXT: $$$BODY',
                'list_comprehension': '[$EXPR for $VAR in $ITER]'
            }
        }
    
    def _identify_relevant_languages(self, issue: str) -> List[str]:
        """Identify which programming languages are relevant to the issue"""
        issue_lower = issue.lower()
        detected_languages = set(self.codebase_index.get('language_map', {}).keys())
        
        # Language hints from issue text
        language_hints = {
            'rust': ['rust', 'cargo', 'sqlx', 'tokio', 'axum', 'serde', 'database', 'connection', 'pool'],
            'javascript': ['javascript', 'js', 'node', 'npm', 'react', 'express'],
            'typescript': ['typescript', 'ts'],
            'python': ['python', 'py', 'django', 'flask', 'fastapi'],
            'solidity': ['solidity', 'contract', 'ethereum', 'blockchain', 'smart contract'],
            'shell': ['bash', 'shell', 'script', 'deployment']
        }
        
        relevant_languages = []
        
        # Check for explicit language mentions
        for language, hints in language_hints.items():
            if language in detected_languages and any(hint in issue_lower for hint in hints):
                relevant_languages.append(language)
        
        # CRITICAL FIX: If no specific language detected, prioritize based on issue type
        if not relevant_languages:
            if any(db_term in issue_lower for db_term in ['database', 'connection', 'pool', 'sql']):
                if 'rust' in detected_languages:
                    relevant_languages = ['rust']
                else:
                    relevant_languages = list(detected_languages)[:3]
            elif any(api_term in issue_lower for api_term in ['api', 'endpoint', 'handler', 'auth', 'authentication']):
                if 'rust' in detected_languages:
                    relevant_languages = ['rust']  # Rust APIs are common in this codebase
                else:
                    relevant_languages = list(detected_languages)[:3]
            else:
                relevant_languages = list(detected_languages)[:3]
        
        print(f"🔍 Relevant languages detected: {relevant_languages}")
        return relevant_languages
    
    def _identify_relevant_directories(self, issue: str) -> List[str]:
        """Identify relevant directories based on issue content"""
        issue_lower = issue.lower()
        directory_purposes = self.codebase_index.get('directory_purposes', {})
        
        # Issue type to directory mapping
        issue_directory_hints = {
            'api': ['api', 'endpoint', 'handler', 'route', 'server'],
            'database': ['database', 'sql', 'query', 'migration', 'connection', 'pool'],
            'cli': ['cli', 'command', 'terminal'],
            'contracts': ['contract', 'blockchain', 'ethereum', 'smart contract'],
            'tests': ['test', 'testing', 'spec'],
            'frontend': ['ui', 'frontend', 'web', 'client'],
            'config': ['config', 'configuration', 'settings']
        }
        
        relevant_dirs = []
        
        for purpose, hints in issue_directory_hints.items():
            if any(hint in issue_lower for hint in hints):
                # Find directories with this purpose
                matching_dirs = [dir_path for dir_path, dir_purpose in directory_purposes.items() 
                               if dir_purpose == purpose]
                relevant_dirs.extend(matching_dirs)
        
        # If no specific directories found, include main directories
        if not relevant_dirs:
            relevant_dirs = list(directory_purposes.keys())[:5]  # Top 5 directories
        
        return relevant_dirs
    
    def _generate_language_specific_queries(self, issue: str, language: str, directories: List[str]) -> List[str]:
        """Generate AST queries specific to a programming language"""
        if language not in self.language_patterns:
            return []
        
        patterns = self.language_patterns[language]
        issue_lower = issue.lower()
        queries = []
        
        # Build directory filter
        dir_filter = self._build_directory_filter(directories, language)
        
        # Core function patterns (always useful)
        if 'function' in issue_lower or 'method' in issue_lower or 'fn' in issue_lower:
            queries.append(f"ast-grep --pattern '{patterns['function_def']}' {dir_filter}")
        
        # API and endpoint patterns
        if any(api_term in issue_lower for api_term in ['api', 'endpoint', 'handler', 'route', 'auth', 'authentication']):
            if language == 'rust':
                # Use proper AST patterns for Rust web frameworks
                if 'router_pattern' in patterns:
                    queries.append(f"ast-grep --pattern '{patterns['router_pattern']}' {dir_filter}")
                if 'auth_service' in patterns:
                    queries.append(f"ast-grep --pattern '{patterns['auth_service']}' {dir_filter}")
                # Look for handler functions
                queries.append(f"ast-grep --pattern '{patterns['function_def']}' {dir_filter}")
        
        # Database-specific patterns
        if any(db_term in issue_lower for db_term in ['database', 'connection', 'pool', 'sql']):
            if language == 'rust':
                if 'connection_pool' in patterns:
                    queries.append(f"ast-grep --pattern '{patterns['connection_pool']}' {dir_filter}")
                if 'max_connections' in patterns:
                    queries.append(f"ast-grep --pattern '{patterns['max_connections']}' {dir_filter}")
        
        # Error and issue patterns
        if any(error_term in issue_lower for error_term in ['error', 'fix', 'bug', 'issue', 'problem']):
            if language == 'rust':
                if 'result_type' in patterns:
                    queries.append(f"ast-grep --pattern '{patterns['result_type']}' {dir_filter}")
        
        # Async patterns
        if any(async_term in issue_lower for async_term in ['async', 'await', 'concurrent']):
            if 'async_function' in patterns:
                queries.append(f"ast-grep --pattern '{patterns['async_function']}' {dir_filter}")
        
        return queries
    
    def _generate_cross_language_queries(self, issue: str, directories: List[str]) -> List[str]:
        """Generate queries that work across multiple languages"""
        issue_lower = issue.lower()
        queries = []
        
        # Configuration file patterns
        if any(config_term in issue_lower for config_term in ['config', 'setting', 'env']):
            queries.extend([
                f"ast-grep --pattern '$key = $value' {'/'.join(directories)}/**/*.toml",
                f"ast-grep --pattern '\"$key\": $value' {'/'.join(directories)}/**/*.json",
            ])
        
        # Error handling patterns
        if any(error_term in issue_lower for error_term in ['error', 'exception', 'panic']):
            queries.extend([
                f"ast-grep --pattern 'Result<$ok, $err>' {'/'.join(directories)}/**/*.rs",
                f"ast-grep --pattern 'try {{ $$$ }} catch ($err) {{ $$$ }}' {'/'.join(directories)}/**/*.js",
                f"ast-grep --pattern 'except $exception:' {'/'.join(directories)}/**/*.py"
            ])
        
        return queries
    
    def _build_directory_filter(self, directories: List[str], language: str) -> str:
        """Build directory filter for AST queries"""
        if not directories:
            return ""
        
        # For single directory, return simple directory path
        if len(directories) == 1:
            return f"--lang {language} {directories[0]}/"
        
        # For multiple directories, use the first one
        return f"--lang {language} {directories[0]}/"
    
    def _get_language_extension(self, language: str) -> str:
        """Get primary file extension for language"""
        extensions = {
            'rust': 'rs',
            'javascript': 'js', 
            'typescript': 'ts',
            'python': 'py',
            'solidity': 'sol',
            'go': 'go',
            'shell': 'sh'
        }
        return extensions.get(language, '*')

# ============================================================================
# LAYER 3: AST QUERY EXECUTOR & RESULTS ANALYZER
# ============================================================================

class ASTQueryExecutor:
    """Executes AST-grep queries and analyzes results for file relevance"""
    
    def __init__(self, root_path: str):
        self.root_path = Path(root_path).resolve()
        
    def execute_ast_queries(self, queries: List[str]) -> Dict[str, List[Dict]]:
        """Execute AST-grep queries and collect structured results"""
        print(f"⚡ Executing {len(queries)} AST queries...")
        
        results = {}
        
        for i, query in enumerate(queries, 1):
            print(f"   Query {i}/{len(queries)}: {query[:60]}...")
            try:
                query_result = self._execute_single_query(query)
                results[query] = query_result
            except Exception as e:
                print(f"   ⚠️ Query failed: {e}")
                results[query] = []
        
        print(f"✅ Executed all AST queries")
        return results
    
    def _execute_single_query(self, query: str) -> List[Dict]:
        """Execute a single AST-grep query and parse results"""
        try:
            # Execute ast-grep command
            result = subprocess.run(
                query,
                shell=True,
                cwd=self.root_path,
                capture_output=True,
                text=True,
                timeout=30  # 30 second timeout per query
            )
            
            if result.returncode == 0:
                if result.stdout.strip():
                    print(f"   ✅ Found {len(result.stdout.strip().split(chr(10)))} matches")
                    return self._parse_ast_grep_output(result.stdout)
                else:
                    print(f"   ❌ No matches found")
                    return []
            else:
                print(f"   ❌ AST-grep error (code {result.returncode})")
                print(f"   📜 stderr: {result.stderr}")
                print(f"   📜 stdout: {result.stdout}")
                return []
                
        except subprocess.TimeoutExpired:
            print(f"   Query timed out: {query[:50]}...")
            return []
        except Exception as e:
            print(f"   Query execution error: {e}")
            return []
    
    def _parse_ast_grep_output(self, output: str) -> List[Dict]:
        """Parse ast-grep output into structured format"""
        matches = []
        
        for line in output.strip().split('\n'):
            if line and ':' in line:
                # Parse format: file_path:line_number:matched_content
                parts = line.split(':', 2)
                if len(parts) >= 2:
                    matches.append({
                        'file': parts[0],
                        'line': parts[1] if len(parts) > 1 else '1',
                        'content': parts[2] if len(parts) > 2 else line,
                        'match_type': 'ast_pattern'
                    })
        
        return matches

class IntelligentFileSelector:
    """Analyzes AST results and tree index to select most relevant files"""
    
    def __init__(self, codebase_index: Dict):
        self.codebase_index = codebase_index
        
    def select_relevant_files(self, issue: str, ast_results: Dict[str, List[Dict]]) -> List[str]:
        """Select most relevant files based on AST results and codebase analysis"""
        print("🎯 Analyzing AST results for file selection...")
        
        # Extract all files mentioned in AST results
        file_scores = Counter()
        file_contexts = {}
        
        # Score files based on AST match quality and frequency
        for query, matches in ast_results.items():
            for match in matches:
                file_path = match['file']
                
                # Score based on match type and context
                score = self._calculate_file_score(match, query, issue)
                file_scores[file_path] += score
                
                # Store context for analysis
                if file_path not in file_contexts:
                    file_contexts[file_path] = []
                file_contexts[file_path].append({
                    'query': query,
                    'match': match,
                    'score': score
                })
        
        # Apply additional scoring based on tree structure
        file_scores = self._apply_tree_structure_scoring(file_scores, issue)
        
        # Select top files with diversity
        selected_files = self._select_diverse_files(file_scores, file_contexts, max_files=3)
        
        print(f"✅ Selected {len(selected_files)} files: {selected_files}")
        return selected_files
    
    def _calculate_file_score(self, match: Dict, query: str, issue: str) -> float:
        """Calculate relevance score for a file based on AST match"""
        base_score = 1.0
        
        # Higher score for implementation files vs usage
        if any(impl_pattern in match.get('content', '') 
               for impl_pattern in ['fn ', 'struct ', 'impl ', 'class ', 'def ']):
            base_score += 2.0
        
        # Higher score for entry points
        if any(entry_pattern in match['file'] 
               for entry_pattern in ['main.', 'lib.', 'mod.', 'index.', 'app.']):
            base_score += 1.5
        
        # Score based on issue-query alignment
        issue_lower = issue.lower()
        query_lower = query.lower()
        
        # Database connection specific scoring
        if 'connection' in issue_lower or 'pool' in issue_lower:
            if any(db_pattern in query_lower 
                   for db_pattern in ['pgpooloptions', 'connection_pool', 'max_connections']):
                base_score += 3.0
        
        # Function definition vs usage scoring
        if 'function' in issue_lower:
            if 'fn ' in match.get('content', '') or 'function ' in match.get('content', ''):
                base_score += 2.0
        
        return base_score
    
    def _apply_tree_structure_scoring(self, file_scores: Counter, issue: str) -> Counter:
        """Apply additional scoring based on tree structure analysis"""
        directory_purposes = self.codebase_index.get('directory_purposes', {})
        entry_points = self.codebase_index.get('entry_points', [])
        
        adjusted_scores = file_scores.copy()
        
        for file_path, score in file_scores.items():
            # Boost scores for files in relevant directories
            file_dir = str(Path(file_path).parent)
            
            if file_dir in directory_purposes:
                purpose = directory_purposes[file_dir]
                
                # Issue-purpose alignment bonuses
                issue_lower = issue.lower()
                if purpose == 'database' and any(db_term in issue_lower 
                                               for db_term in ['database', 'connection', 'sql', 'pool']):
                    adjusted_scores[file_path] += 2.0
                elif purpose == 'api' and any(api_term in issue_lower 
                                            for api_term in ['api', 'endpoint', 'handler', 'server']):
                    adjusted_scores[file_path] += 2.0
                elif purpose == 'cli' and any(cli_term in issue_lower 
                                            for cli_term in ['cli', 'command', 'terminal']):
                    adjusted_scores[file_path] += 2.0
            
            # Boost entry points
            if file_path in entry_points:
                adjusted_scores[file_path] += 1.5
        
        return adjusted_scores
    
    def _select_diverse_files(self, file_scores: Counter, file_contexts: Dict, max_files: int = 3) -> List[str]:
        """Select diverse set of files avoiding duplicates from same directory"""
        selected_files = []
        selected_directories = set()
        
        # Sort by score descending
        sorted_files = file_scores.most_common()
        
        for file_path, score in sorted_files:
            if len(selected_files) >= max_files:
                break
                
            file_dir = str(Path(file_path).parent)
            
            # Ensure diversity - avoid too many files from same directory
            if file_dir not in selected_directories or len(selected_files) < max_files // 2:
                selected_files.append(file_path)
                selected_directories.add(file_dir)
        
        # If we don't have enough files, fill with highest scoring regardless of directory
        if len(selected_files) < max_files:
            for file_path, score in sorted_files:
                if file_path not in selected_files:
                    selected_files.append(file_path)
                    if len(selected_files) >= max_files:
                        break
        
        return selected_files

# ============================================================================
# LAYER 4: DYNAMIC I2P ENGINE (REPLACES HARDCODED ENSEMBLE)  
# ============================================================================

class DynamicI2P:
    """Universal dynamic codebase explorer using AST-powered discovery"""
    
    def __init__(self, root_path: str = "../"):
        self.root_path = Path(root_path).resolve()
        print(f"🗂️ Root path resolved to: {self.root_path}")
        self.models = {
            "bulk_processor": "nvidia/nemotron-nano-9b-v2",  # High-volume, cost-efficient
            "code_specialist": "anthropic/claude-sonnet-4",   # Precision analysis
            "reasoning": "x-ai/grok-4", 
            "validator": "openai/o3"
        }
        
        # Cost tracking
        self.model_costs = {
            "nvidia/nemotron-nano-9b-v2": {"input": 0.04, "output": 0.16},  # per million tokens
            "anthropic/claude-sonnet-4": {"input": 3.00, "output": 15.00},
            "x-ai/grok-4": {"input": 5.00, "output": 15.00},
            "openai/o3": {"input": 15.00, "output": 60.00}
        }
        
        print("🚀 Initializing Dynamic I2P System...")
        
        # Generate dynamic codebase index
        self.tree_indexer = DynamicTreeIndexer(root_path)
        self.codebase_index = self.tree_indexer.generate_codebase_index()
        
        # Initialize file discovery components  
        self.file_selector = IntelligentFileSelector(self.codebase_index)
        
        # LLM enhancement configuration
        self.enable_llm_enhancement = False  # Optional enhancement layer
        self.cost_tracking = {"total_input_tokens": 0, "total_output_tokens": 0, "total_cost_usd": 0.0}
        
        # Domain-specific templates and patterns
        self.domain_templates = self._initialize_domain_templates()
        self.business_impact_mapping = self._initialize_business_mapping()
        
        print("✅ Dynamic I2P initialized with tree-guided AST discovery")
    
    def _initialize_domain_templates(self) -> Dict:
        """Initialize domain-specific templates for expert-level recommendations"""
        return {
            "zk_observability": {
                "keywords": ["zk", "proof", "batch", "sindri", "sp1", "zkvm", "observability", "metrics", "tracing"],
                "required_patterns": ["batch_processing", "sindri_integration", "proof_generation"],
                "business_metrics": ["cost_per_proof", "generation_time", "success_rate", "sindri_spend"],
                "implementation_sections": ["sindri_instrumentation", "zkvm_metrics", "pipeline_stages"],
                "critical_functions": ["process_batch", "generate_proof", "submit_to_sindri", "poll_sindri_status"]
            },
            
            "database_optimization": {
                "keywords": ["database", "sql", "connection", "pool", "query", "performance", "migration"],
                "required_patterns": ["connection_pooling", "query_performance", "transaction_management"],
                "business_metrics": ["query_latency", "connection_usage", "deadlock_rate", "throughput"],
                "implementation_sections": ["pool_configuration", "query_optimization", "monitoring"],
                "critical_functions": ["get_pool", "execute_query", "create_connection", "health_check"]
            },
            
            "api_security": {
                "keywords": ["api", "auth", "security", "rate", "limit", "jwt", "login", "endpoint"],
                "required_patterns": ["authentication", "rate_limiting", "input_validation", "middleware"],
                "business_metrics": ["attack_attempts", "auth_success_rate", "security_events", "response_time"],
                "implementation_sections": ["auth_middleware", "rate_limit_config", "security_headers"],
                "critical_functions": ["authenticate", "validate_token", "check_rate_limit", "handle_request"]
            },
            
            "error_handling": {
                "keywords": ["error", "exception", "panic", "result", "handling", "logging", "recovery"],
                "required_patterns": ["error_propagation", "logging", "recovery_strategies"],
                "business_metrics": ["error_rate", "recovery_time", "user_impact", "system_stability"],
                "implementation_sections": ["error_types", "logging_strategy", "recovery_mechanisms"],
                "critical_functions": ["handle_error", "log_error", "recover_from_failure", "validate_input"]
            },
            
            "loan_lifecycle_management": {
                "keywords": ["loan", "originate", "payment", "closeout", "borrower", "lender", "principal", "interest", "balance"],
                "required_patterns": ["loan_state_management", "payment_processing", "interest_calculation", "kyc_validation"],
                "business_metrics": ["loan_origination_time", "payment_success_rate", "outstanding_balance_accuracy", "closeout_efficiency"],
                "implementation_sections": ["loan_handlers", "payment_validation", "state_transitions", "balance_updates"],
                "critical_functions": ["originate_loan", "make_payment", "closeout_loan", "calculate_interest", "update_loan_status"]
            },
            
            "usdc_operations": {
                "keywords": ["usdc", "deposit", "withdrawal", "balance", "ethereum", "contract", "wallet", "transfer"],
                "required_patterns": ["deposit_monitoring", "withdrawal_processing", "balance_synchronization", "ethereum_integration"],
                "business_metrics": ["deposit_confirmation_time", "withdrawal_success_rate", "balance_accuracy", "gas_efficiency"],
                "implementation_sections": ["deposit_handlers", "withdrawal_service", "ethereum_client", "balance_tracking"],
                "critical_functions": ["process_deposit", "initiate_withdrawal", "update_balance", "monitor_ethereum", "validate_transaction"]
            },
            
            "liquidity_pool_management": {
                "keywords": ["pool", "liquidity", "investor", "yield", "ltv", "credit", "matching", "deployment"],
                "required_patterns": ["pool_creation", "liquidity_matching", "yield_calculation", "risk_assessment"],
                "business_metrics": ["pool_utilization_rate", "matching_efficiency", "yield_accuracy", "investor_returns"],
                "implementation_sections": ["pool_handlers", "matching_algorithms", "yield_engine", "risk_monitoring"],
                "critical_functions": ["create_pool", "find_matching_pools", "calculate_yield", "assess_risk", "deploy_credit"]
            },
            
            "kyc_compliance": {
                "keywords": ["kyc", "compliance", "verification", "identity", "aml", "regulatory", "audit", "documentation"],
                "required_patterns": ["identity_verification", "compliance_checking", "audit_trail", "regulatory_reporting"],
                "business_metrics": ["kyc_completion_rate", "verification_time", "compliance_score", "audit_readiness"],
                "implementation_sections": ["kyc_service", "compliance_engine", "audit_logging", "regulatory_reports"],
                "critical_functions": ["submit_kyc", "verify_identity", "check_compliance", "generate_audit_trail", "validate_regulatory_status"]
            },
            
            "sp1_zkvm_integration": {
                "keywords": ["sp1", "zkvm", "proof", "program", "risc", "circuit", "verification", "nullifier"],
                "required_patterns": ["zkvm_execution", "proof_generation", "circuit_deployment", "verification_logic"],
                "business_metrics": ["proof_generation_time", "verification_success_rate", "circuit_efficiency", "zkvm_performance"],
                "implementation_sections": ["zkvm_program", "proof_service", "circuit_management", "verification_handlers"],
                "critical_functions": ["execute_zkvm_program", "generate_sp1_proof", "verify_proof", "deploy_circuit", "process_nullifiers"]
            }
        }
    
    def _initialize_business_mapping(self) -> Dict:
        """Initialize business impact mapping for arda-credit specific technical changes"""
        return {
            "zk_proof_pipeline": {
                "cost_drivers": ["sindri_api_calls", "proof_generation_latency", "batch_processing_failures", "sp1_program_complexity"],
                "performance_kpis": ["proof_generation_time", "batch_throughput", "verification_success_rate", "loan_settlement_latency"],
                "revenue_impact": ["loan_processing_capacity", "user_trust_in_privacy", "regulatory_compliance", "scalable_credit_operations"],
                "typical_costs": {"sindri_proof_cost": 0.50, "batch_failure_cost": 25, "proof_timeout_cost": 10, "monthly_sindri_budget": 500}
            },
            
            "database_operations": {
                "cost_drivers": ["postgresql_connection_limits", "loan_query_performance", "merkle_tree_updates", "kyc_data_access"],
                "performance_kpis": ["loan_transaction_latency", "concurrent_user_capacity", "data_consistency", "usdc_balance_accuracy"],
                "revenue_impact": ["loan_origination_throughput", "user_experience_quality", "operational_efficiency"],
                "typical_costs": {"db_optimization": 300, "connection_pool_tuning": 150, "indexing_improvements": 200, "monitoring_setup": 100}
            },
            
            "api_security": {
                "cost_drivers": ["kyc_verification_overhead", "jwt_token_validation", "rate_limiting_complexity", "audit_trail_storage"],
                "performance_kpis": ["authentication_latency", "security_incident_count", "api_uptime", "compliance_score"],
                "revenue_impact": ["regulatory_compliance", "user_trust", "system_availability", "audit_readiness"],
                "typical_costs": {"security_audit": 5000, "kyc_integration": 1000, "auth_optimization": 400, "compliance_monitoring": 600}
            },
            
            "loan_lifecycle_management": {
                "cost_drivers": ["loan_state_transitions", "payment_processing_complexity", "closeout_verification", "interest_calculations"],
                "performance_kpis": ["loan_origination_time", "payment_processing_speed", "closeout_accuracy", "outstanding_balance_precision"],
                "revenue_impact": ["loan_volume_capacity", "payment_reliability", "investor_confidence", "lender_satisfaction"],
                "typical_costs": {"loan_processing_optimization": 800, "payment_verification": 300, "state_management": 400, "reporting_automation": 250}
            },
            
            "usdc_operations": {
                "cost_drivers": ["ethereum_gas_costs", "deposit_confirmation_delays", "withdrawal_processing_time", "balance_synchronization"],
                "performance_kpis": ["deposit_confirmation_time", "withdrawal_success_rate", "balance_accuracy", "blockchain_sync_reliability"],
                "revenue_impact": ["user_liquidity_confidence", "deposit_conversion_rate", "withdrawal_satisfaction", "platform_trust"],
                "typical_costs": {"gas_optimization": 200, "ethereum_monitoring": 150, "deposit_automation": 300, "withdrawal_security": 400}
            },
            
            "liquidity_pool_management": {
                "cost_drivers": ["pool_matching_algorithms", "yield_calculation_complexity", "ltv_monitoring", "investor_reporting"],
                "performance_kpis": ["pool_utilization_rate", "matching_efficiency", "yield_accuracy", "risk_assessment_speed"],
                "revenue_impact": ["investor_returns", "platform_fee_revenue", "pool_growth_rate", "credit_deployment_efficiency"],
                "typical_costs": {"matching_optimization": 600, "yield_calculation": 200, "risk_monitoring": 350, "investor_dashboard": 400}
            }
        }
    
    def _detect_domain(self, issue: str, selected_files: List[str]) -> str:
        """Detect the primary domain based on issue keywords and files"""
        issue_lower = issue.lower()
        
        # Check each domain template for keyword matches
        best_match = {"domain": "general", "score": 0}
        
        for domain, template in self.domain_templates.items():
            score = 0
            
            # Score based on issue keywords
            for keyword in template["keywords"]:
                if keyword in issue_lower:
                    score += 2
            
            # Score based on file patterns
            for file_path in selected_files:
                file_lower = file_path.lower()
                for keyword in template["keywords"]:
                    if keyword in file_lower:
                        score += 1
            
            if score > best_match["score"]:
                best_match = {"domain": domain, "score": score}
        
        detected_domain = best_match["domain"]
        print(f"🎯 Detected domain: {detected_domain} (score: {best_match['score']})")
        return detected_domain
    
    def _select_optimal_model(self, issue: str, code_size: int, complexity: str) -> str:
        """Select the most cost-efficient model based on task requirements"""
        
        # High-volume or large context tasks → Use Nemotron Nano
        if code_size > 50000:
            print(f"📊 Selected bulk processor (code size: {code_size:,} chars)")
            return self.models["bulk_processor"]
        
        # Complex reasoning tasks → Use Claude Sonnet-4
        complex_keywords = ["architecture", "security", "critical", "production", "design patterns", "refactor"]
        if any(keyword in issue.lower() for keyword in complex_keywords):
            print(f"🎯 Selected precision specialist (complex reasoning required)")
            return self.models["code_specialist"]
        
        # Budget-conscious default → Use Nemotron Nano
        if complexity in ["Low", "Medium"]:
            print(f"💰 Selected bulk processor (cost optimization)")
            return self.models["bulk_processor"]
        
        # High complexity → Use Claude Sonnet-4
        print(f"🧠 Selected precision specialist (high complexity)")
        return self.models["code_specialist"]
    
    def _estimate_cost(self, model: str, input_tokens: int, estimated_output_tokens: int = 500) -> float:
        """Estimate cost for model usage"""
        costs = self.model_costs.get(model, {"input": 3.0, "output": 15.0})
        input_cost = (input_tokens / 1_000_000) * costs["input"]
        output_cost = (estimated_output_tokens / 1_000_000) * costs["output"]
        return input_cost + output_cost
    
    def _track_usage(self, model: str, input_tokens: int, output_tokens: int):
        """Track model usage and costs"""
        cost = self._estimate_cost(model, input_tokens, output_tokens)
        self.cost_tracking["total_input_tokens"] += input_tokens
        self.cost_tracking["total_output_tokens"] += output_tokens
        self.cost_tracking["total_cost_usd"] += cost
        
        print(f"💰 Usage: {input_tokens:,} in + {output_tokens:,} out = ${cost:.4f} ({model.split('/')[-1]})")
    
    def analyze_issue(self, issue: str) -> Dict:
        """Universal issue analysis using dynamic discovery"""
        print(f"🎯 Dynamic Analysis: {issue[:60]}...")
        start_time = time.time()
        
        try:
            # Stage 1: Intelligent file discovery using tree analysis
            selected_files = self._intelligent_file_discovery(issue)
            
            if not selected_files:
                print("⚠️ No relevant files found, using fallback analysis")
                return self._fallback_structure_analysis(issue, start_time)
            
            print(f"📁 Discovered {len(selected_files)} relevant files: {selected_files}")
            
            # Stage 2: Detect domain early for optimized extraction
            domain = self._detect_domain(issue, selected_files)
            
            # Stage 2a: Apply domain-specific code extraction optimization
            extraction_strategy = self._get_domain_specific_extraction_strategy(domain, selected_files)
            
            # Stage 2b: Extract comprehensive code context using optimized code2prompt
            code_context = self._extract_code_context_with_domain_optimization(selected_files, extraction_strategy, domain)
            
            if not code_context:
                print("⚠️ Failed to extract code context, using fallback")  
                return self._fallback_structure_analysis(issue, start_time)
            
            # Stage 3: Code Pattern Analysis (Enhanced Intelligence Layer)
            pattern_analysis = self._analyze_code_patterns(code_context, domain, issue) if self.enable_llm_enhancement else {}
            
            # Stage 4: Hybrid LLM Enhancement Layer (Now domain-aware)
            llm_enhancement = self._llm_enhance_analysis_with_patterns(issue, selected_files, code_context, pattern_analysis, domain)
            
            # Stage 5: Generate expert-level prompt with implementation guidance
            optimized_prompt = self._generate_expert_prompt(issue, selected_files, code_context, pattern_analysis, llm_enhancement, domain)
            
            # Stage 6: Final quality evaluation layer
            evaluation_metadata = {
                "detected_domain": domain,
                "selected_files": selected_files,
                "pattern_analysis": pattern_analysis,
                "llm_enhancement": llm_enhancement
            }
            quality_evaluation = self._evaluate_prompt_quality(issue, optimized_prompt, evaluation_metadata)
            
            processing_time = time.time() - start_time
            
            return {
                "issue": issue,
                "method": "expert_pattern_analysis_with_evaluation",
                "codebase_analysis": self._summarize_codebase_analysis(),
                "selected_files": selected_files,
                "file_discovery_method": "tree_analysis_with_code2prompt", 
                "detected_domain": domain,
                "pattern_analysis": pattern_analysis,
                "code_context_length": len(code_context),
                "llm_enhancement": llm_enhancement,
                "quality_evaluation": quality_evaluation,
                "optimized_prompt": optimized_prompt,
                "processing_time": processing_time,
                "cost_tracking": self.cost_tracking.copy(),
                "success": True
            }
            
        except Exception as e:
            print(f"❌ Dynamic analysis failed: {e}")
            return {"success": False, "error": str(e)}
    
    def _fallback_structure_analysis(self, issue: str, start_time: float) -> Dict:
        """Fallback using directory structure analysis when AST fails"""
        print("🔄 Using structure-based fallback analysis")
        
        # Use directory purposes to find relevant files
        directory_purposes = self.codebase_index.get('directory_purposes', {})
        entry_points = self.codebase_index.get('entry_points', [])
        
        # Simple issue-directory matching
        issue_lower = issue.lower()
        relevant_files = []
        
        # Check entry points first
        relevant_files.extend(entry_points[:2])
        
        # Add files from relevant directories
        for dir_path, purpose in directory_purposes.items():
            if purpose == 'database' and any(term in issue_lower for term in ['database', 'sql', 'connection']):
                # Find main files in database directory
                db_files = self._find_main_files_in_directory(dir_path)
                relevant_files.extend(db_files[:1])
            elif purpose == 'api' and any(term in issue_lower for term in ['api', 'server', 'endpoint']):
                api_files = self._find_main_files_in_directory(dir_path)
                relevant_files.extend(api_files[:1])
        
        # Remove duplicates and limit
        selected_files = list(dict.fromkeys(relevant_files))[:3]
        
        if not selected_files:
            selected_files = entry_points[:3]  # Ultimate fallback
        
        code_context = self._extract_code_context(selected_files)
        
        # Optional LLM enhancement for fallback
        llm_enhancement = self._llm_enhance_analysis(issue, selected_files, code_context, {})
        
        optimized_prompt = self._generate_dynamic_prompt(issue, selected_files, code_context, {}, llm_enhancement)
        
        processing_time = time.time() - start_time
        
        return {
            "issue": issue,
            "method": "structure_fallback",
            "selected_files": selected_files,
            "code_context_length": len(code_context),
            "llm_enhancement": llm_enhancement,
            "optimized_prompt": optimized_prompt,
            "processing_time": processing_time,
            "success": True
        }
    
    def _find_main_files_in_directory(self, dir_path: str) -> List[str]:
        """Find main implementation files in a directory"""
        main_files = []
        full_dir_path = Path(self.root_path) / dir_path
        
        if full_dir_path.exists():
            # Look for common entry point files
            entry_patterns = ['lib.rs', 'mod.rs', 'main.rs', '__init__.py', 'index.js']
            
            for pattern in entry_patterns:
                file_path = full_dir_path / pattern
                if file_path.exists():
                    rel_path = str(file_path.relative_to(Path(self.root_path)))
                    main_files.append(rel_path)
        
        return main_files
    
    def _intelligent_file_discovery(self, issue: str) -> List[str]:
        """Smart file discovery using tree analysis and issue keywords"""
        print("🔍 Starting intelligent file discovery...")
        
        directory_purposes = self.codebase_index.get('directory_purposes', {})
        entry_points = self.codebase_index.get('entry_points', [])
        language_map = self.codebase_index.get('language_map', {})
        
        issue_lower = issue.lower()
        selected_files = []
        
        # Priority 1: Dynamic file discovery with existence checking and filesystem search
        def get_existing_files(file_list):
            """Filter file list to only include files that actually exist"""
            existing = []
            for file_path in file_list:
                full_path = Path(self.root_path) / file_path
                if full_path.exists():
                    existing.append(file_path)
            return existing
        
        def discover_files_by_pattern(directory: str, patterns: List[str]) -> List[str]:
            """Dynamically discover files in directory matching naming patterns"""
            discovered = []
            dir_path = Path(self.root_path) / directory
            if not dir_path.exists():
                return discovered
            
            for file_path in dir_path.rglob("*.rs"):
                file_name = file_path.name.lower()
                if any(pattern in file_name or pattern in str(file_path).lower() for pattern in patterns):
                    rel_path = str(file_path.relative_to(Path(self.root_path)))
                    discovered.append(rel_path)
            
            # Also check for Solidity files
            for file_path in dir_path.rglob("*.sol"):
                file_name = file_path.name.lower()
                if any(pattern in file_name or pattern in str(file_path).lower() for pattern in patterns):
                    rel_path = str(file_path.relative_to(Path(self.root_path)))
                    discovered.append(rel_path)
            
            return discovered[:3]  # Limit to top 3 matches per directory

        # Combine hardcoded known files with dynamic discovery
        keyword_mappings = {}
        
        # ZK/Proof keywords - combine known files + dynamic discovery
        zk_files = get_existing_files([
            'api/src/batch_processor.rs', 'api/src/unified_batch_service.rs', 
            'lib/src/proof.rs', 'program/src/main.rs'
        ])
        zk_files.extend(discover_files_by_pattern('api/src', ['batch', 'proof', 'sindri']))
        zk_files.extend(discover_files_by_pattern('lib/src', ['proof', 'zkvm']))
        zk_files.extend(discover_files_by_pattern('program/src', ['main', 'zkvm']))
        keyword_mappings['zk|proof|batch|sindri|sp1|zkvm'] = list(dict.fromkeys(zk_files))
        
        # Database keywords - combine known files + dynamic discovery
        db_files = get_existing_files([
            'db/src/db.rs', 'db/src/lib.rs', 'db/src/ads_service.rs', 'db/src/merkle_tree.rs'
        ])
        db_files.extend(discover_files_by_pattern('db/src', ['db', 'connection', 'pool', 'service']))
        keyword_mappings['database|sql|connection|pool|migration'] = list(dict.fromkeys(db_files))
        
        # API/Server keywords - combine known files + dynamic discovery
        api_files = get_existing_files([
            'api/src/router.rs', 'api/src/server.rs', 'api/src/lib.rs'
        ])
        api_files.extend(discover_files_by_pattern('api/src', ['handler', 'route', 'server', 'endpoint']))
        keyword_mappings['api|endpoint|handler|route|server'] = list(dict.fromkeys(api_files))
        
        # Authentication/Security/KYC - combine known files + dynamic discovery
        auth_files = get_existing_files([
            'api/src/auth.rs', 'api/src/authentication_handlers.rs', 'api/src/kyc_service.rs'
        ])
        auth_files.extend(discover_files_by_pattern('api/src', ['auth', 'kyc', 'security', 'jwt']))
        keyword_mappings['auth|security|jwt|login|kyc'] = list(dict.fromkeys(auth_files))
        
        # Smart contracts - combine known files + dynamic discovery
        contract_files = get_existing_files([
            'contracts/src/Loan.sol', 'contracts/src/EventHelpers.sol', 
            'contracts/src/interfaces/IStateManager.sol', 'ethereum-client/src/client.rs'
        ])
        contract_files.extend(discover_files_by_pattern('contracts/src', ['loan', 'event', 'state', 'manager']))
        contract_files.extend(discover_files_by_pattern('ethereum-client/src', ['client', 'ethereum', 'contract']))
        keyword_mappings['contract|solidity|ethereum|blockchain'] = list(dict.fromkeys(contract_files))
        
        # USDC/Deposit/Withdrawal - combine known files + dynamic discovery
        usdc_files = get_existing_files([
            'api/src/balance_service.rs', 'api/src/withdrawal_service.rs',
            'ethereum-client/src/deposit_monitor.rs', 'ethereum-client/src/withdrawal_monitor.rs'
        ])
        usdc_files.extend(discover_files_by_pattern('api/src', ['balance', 'withdrawal', 'deposit']))
        usdc_files.extend(discover_files_by_pattern('ethereum-client/src', ['deposit', 'withdrawal', 'monitor']))
        keyword_mappings['usdc|deposit|withdrawal|balance|wallet'] = list(dict.fromkeys(usdc_files))
        
        # Liquidity Pool operations - combine known files + dynamic discovery
        pool_files = get_existing_files([
            'api/src/pool_handlers.rs', 'lib/src/pool.rs', 'lib/src/credit_evaluation.rs'
        ])
        pool_files.extend(discover_files_by_pattern('api/src', ['pool', 'liquidity', 'credit']))
        pool_files.extend(discover_files_by_pattern('lib/src', ['pool', 'credit', 'evaluation']))
        keyword_mappings['pool|liquidity|investor|yield|ltv|credit'] = list(dict.fromkeys(pool_files))
        
        # Error handling - combine known files + dynamic discovery
        error_files = get_existing_files([
            'api/src/error.rs', 'db/src/error.rs', 'lib/src/lib.rs'
        ])
        error_files.extend(discover_files_by_pattern('api/src', ['error', 'exception']))
        error_files.extend(discover_files_by_pattern('db/src', ['error']))
        error_files.extend(discover_files_by_pattern('lib/src', ['error']))
        keyword_mappings['error|exception|panic|result'] = list(dict.fromkeys(error_files))
        
        # CLI/Tools - combine known files + dynamic discovery
        cli_files = get_existing_files([
            'cli/src/bin/cli.rs', 'cli/src/lib.rs'
        ])
        cli_files.extend(discover_files_by_pattern('cli/src', ['cli', 'command', 'tool']))
        keyword_mappings['cli|command|tool'] = list(dict.fromkeys(cli_files))
        
        # Observability/Monitoring - combine known files + dynamic discovery
        obs_files = get_existing_files([
            'api/src/telemetry.rs', 'api/src/health_handlers.rs'
        ])
        obs_files.extend(discover_files_by_pattern('api/src', ['telemetry', 'health', 'metrics', 'tracing']))
        keyword_mappings['observability|metrics|tracing|logging|monitor'] = list(dict.fromkeys(obs_files))
        
        # Find files matching issue keywords
        for pattern, files in keyword_mappings.items():
            if any(keyword in issue_lower for keyword in pattern.split('|')):
                for file_path in files:
                    full_path = Path(self.root_path) / file_path
                    if full_path.exists():
                        selected_files.append(file_path)
        
        # Priority 2: Add entry points if relevant
        for entry_point in entry_points[:2]:  # Top 2 entry points
            if entry_point not in selected_files:
                selected_files.append(entry_point)
        
        # Priority 3: Add files from relevant directories
        relevant_dirs = []
        dir_keywords = {
            'api': ['api', 'endpoint', 'handler', 'server', 'auth'],
            'database': ['database', 'sql', 'query', 'migration', 'connection'],
            'contracts': ['contract', 'blockchain', 'ethereum', 'solidity'],
            'cli': ['cli', 'command', 'tool']
        }
        
        for purpose, keywords in dir_keywords.items():
            if any(keyword in issue_lower for keyword in keywords):
                matching_dirs = [dir_path for dir_path, dir_purpose in directory_purposes.items() 
                               if dir_purpose == purpose]
                relevant_dirs.extend(matching_dirs)
        
        # Add main files from relevant directories
        for dir_path in relevant_dirs[:2]:  # Limit to top 2 directories
            main_files = self._find_main_files_in_directory(dir_path)
            selected_files.extend(main_files[:1])  # One file per directory
        
        # Remove duplicates and limit total files
        unique_files = list(dict.fromkeys(selected_files))[:5]  # Max 5 files
        
        print(f"📊 File discovery results: {len(unique_files)} files selected")
        return unique_files
    
    def _extract_code_context_with_code2prompt(self, selected_files: List[str]) -> str:
        """Extract comprehensive code context using optimized code2prompt patterns"""
        print("📄 Extracting code context with optimized code2prompt...")
        
        try:
            # Build optimized include pattern for selected files
            include_pattern = ",".join(selected_files)
            
            # Get initial token count for optimization
            token_check_cmd = [
                'code2prompt', str(self.root_path),
                '--include', include_pattern,
                '--tokens', 'raw',
                '--no-clipboard'
            ]
            
            print(f"🔍 Checking token count for {len(selected_files)} files...")
            token_result = subprocess.run(token_check_cmd, capture_output=True, text=True, timeout=30)
            
            estimated_tokens = 0
            if token_result.returncode == 0 and token_result.stdout.strip().isdigit():
                estimated_tokens = int(token_result.stdout.strip())
                print(f"📊 Estimated tokens: {estimated_tokens:,}")
            
            # Build optimized code2prompt command
            cmd = [
                'code2prompt', str(self.root_path),
                '--include', include_pattern,
                '--line-numbers',
                '--encoding', 'cl100k',  # GPT-4/ChatGPT tokenizer
                '--no-clipboard',  # Performance optimization
                '--output-file', '/tmp/intelligent_i2p_context.md'
            ]
            
            # Add performance exclusions for arda-credit
            arda_exclusions = [
                '**/target/**',
                '**/lib/sp1-contracts/**', 
                '**/lib/forge-std/**',
                '**/lib/openzeppelin-contracts/**',
                '**/examples/**',
                '**/tests/**',
                '**/benches/**'
            ]
            
            for exclusion in arda_exclusions:
                cmd.extend(['--exclude', exclusion])
            
            print(f"🔧 Running optimized code2prompt: {len(selected_files)} files, ~{estimated_tokens:,} tokens")
            start_time = time.time()
            
            result = subprocess.run(cmd, capture_output=True, text=True, cwd=Path(__file__).parent, timeout=120)
            
            extraction_time = time.time() - start_time
            
            if result.returncode == 0:
                with open('/tmp/intelligent_i2p_context.md', 'r') as f:
                    context = f.read()
                    
                print(f"✅ Successfully extracted {len(context):,} characters in {extraction_time:.2f}s")
                print(f"   Performance: {len(context)/extraction_time:.0f} chars/sec")
                
                # Warn if context is very large
                if estimated_tokens > 150000:
                    print(f"⚠️  Large context ({estimated_tokens:,} tokens) may exceed LLM limits")
                
                return context
            else:
                print(f"❌ Optimized code2prompt failed: {result.stderr}")
                print(f"📜 stdout: {result.stdout}")
                
                # Fallback to basic extraction
                return self._extract_code_context_fallback(selected_files)
                
        except subprocess.TimeoutExpired:
            print("⏰ Optimized code2prompt timed out after 120 seconds")
            return self._extract_code_context_fallback(selected_files)
        except Exception as e:
            print(f"❌ Optimized context extraction error: {e}")
            return self._extract_code_context_fallback(selected_files)
    
    def _extract_code_context_fallback(self, selected_files: List[str]) -> str:
        """Fallback code extraction with minimal options"""
        print("🔄 Using fallback code2prompt extraction...")
        
        try:
            include_pattern = ",".join(selected_files)
            
            cmd = [
                'code2prompt', str(self.root_path),
                '--include', include_pattern,
                '--line-numbers',
                '--output-file', '/tmp/fallback_i2p_context.md'
            ]
            
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
            
            if result.returncode == 0:
                with open('/tmp/fallback_i2p_context.md', 'r') as f:
                    context = f.read()
                print(f"✅ Fallback extraction: {len(context):,} characters")
                return context
            else:
                print(f"❌ Fallback extraction failed: {result.stderr}")
                return ""
                
        except Exception as e:
            print(f"❌ Fallback extraction error: {e}")
            return ""
    
    def _get_domain_specific_extraction_strategy(self, domain: str, selected_files: List[str]) -> Dict[str, any]:
        """Get optimized code2prompt strategy based on detected domain"""
        
        base_exclusions = [
            '**/target/**',
            '**/lib/sp1-contracts/**', 
            '**/lib/forge-std/**',
            '**/lib/openzeppelin-contracts/**',
            '**/examples/**',
            '**/tests/**',
            '**/benches/**'
        ]
        
        domain_strategies = {
            "zk_observability": {
                "additional_includes": [
                    "api/src/telemetry.rs",
                    "api/src/health_handlers.rs", 
                    "**/tracing*.rs"
                ],
                "exclusions": base_exclusions + ["**/migrations/**"],
                "token_limit": 150000,
                "focus": "ZK proof pipeline instrumentation"
            },
            
            "sp1_zkvm_integration": {
                "additional_includes": [
                    "program/src/**/*.rs",
                    "**/zkvm*.rs",
                    "**/sp1*.rs"
                ],
                "exclusions": base_exclusions + ["**/api/src/handlers/**"],
                "token_limit": 120000,
                "focus": "SP1 zkVM execution and proof generation"
            },
            
            "database_optimization": {
                "additional_includes": [
                    "db/src/**/*.rs",
                    "api/src/**/*pool*.rs",
                    "**/*connection*.rs"
                ],
                "exclusions": base_exclusions + ["**/contracts/**", "**/program/**"],
                "token_limit": 100000,
                "focus": "Database connection pooling and query optimization"
            },
            
            "loan_lifecycle_management": {
                "additional_includes": [
                    "api/src/*loan*.rs",
                    "lib/src/loan*.rs",
                    "db/src/*loan*.rs"
                ],
                "exclusions": base_exclusions + ["**/ethereum-client/**"],
                "token_limit": 130000,
                "focus": "Loan origination, payments, and lifecycle"
            },
            
            "usdc_operations": {
                "additional_includes": [
                    "api/src/balance_service.rs",
                    "api/src/withdrawal_service.rs",
                    "ethereum-client/**/*.rs",
                    "contracts/src/*.sol"
                ],
                "exclusions": base_exclusions,
                "token_limit": 140000,
                "focus": "USDC deposit/withdrawal and Ethereum integration"
            },
            
            "api_security": {
                "additional_includes": [
                    "api/src/auth*.rs",
                    "api/src/kyc*.rs",
                    "api/src/middleware.rs",
                    "**/*security*.rs"
                ],
                "exclusions": base_exclusions + ["**/contracts/**", "**/program/**"],
                "token_limit": 90000,
                "focus": "Authentication, KYC, and security middleware"
            }
        }
        
        strategy = domain_strategies.get(domain, {
            "additional_includes": [],
            "exclusions": base_exclusions,
            "token_limit": 100000,
            "focus": "General code analysis"
        })
        
        print(f"📋 Domain strategy: {strategy['focus']}")
        print(f"🎯 Token limit: {strategy['token_limit']:,}")
        
        return strategy
    
    def _extract_code_context_with_domain_optimization(self, selected_files: List[str], strategy: Dict[str, any], domain: str) -> str:
        """Extract code context using domain-specific optimization strategy"""
        print("🎯 Extracting code context with domain optimization...")
        
        try:
            # Combine selected files with domain-specific additional includes
            all_includes = selected_files.copy()
            
            # Add domain-specific files that exist
            for additional_pattern in strategy.get("additional_includes", []):
                # Use glob to find matching files
                matching_files = []
                for file_path in Path(self.root_path).rglob("*"):
                    if file_path.is_file():
                        rel_path = str(file_path.relative_to(Path(self.root_path)))
                        # Simple pattern matching (could be enhanced with proper glob)
                        pattern_parts = additional_pattern.replace("**/", "").replace("/**", "").replace("*", "")
                        if pattern_parts in rel_path:
                            matching_files.append(rel_path)
                
                # Add unique matches (limit to prevent explosion)
                for match in matching_files[:3]:
                    if match not in all_includes:
                        all_includes.append(match)
            
            # Build optimized include pattern
            include_pattern = ",".join(all_includes)
            
            print(f"📊 Domain-optimized files: {len(all_includes)} total ({len(selected_files)} base + {len(all_includes) - len(selected_files)} domain-specific)")
            
            # Get token count for optimization
            token_check_cmd = [
                'code2prompt', str(self.root_path),
                '--include', include_pattern,
                '--tokens', 'raw',
                '--no-clipboard'
            ]
            
            # Add domain-specific exclusions
            for exclusion in strategy.get("exclusions", []):
                token_check_cmd.extend(['--exclude', exclusion])
            
            token_result = subprocess.run(token_check_cmd, capture_output=True, text=True, timeout=30)
            
            estimated_tokens = 0
            if token_result.returncode == 0 and token_result.stdout.strip().isdigit():
                estimated_tokens = int(token_result.stdout.strip())
                print(f"📊 Domain-optimized tokens: {estimated_tokens:,} (limit: {strategy.get('token_limit', 100000):,})")
            
            # Check if we exceed domain token limit
            token_limit = strategy.get('token_limit', 100000)
            if estimated_tokens > token_limit:
                print(f"⚠️  Token count {estimated_tokens:,} exceeds domain limit {token_limit:,}, using selective extraction")
                # Fall back to just selected files for token management
                all_includes = selected_files
                include_pattern = ",".join(all_includes)
            
            # Build optimized extraction command
            cmd = [
                'code2prompt', str(self.root_path),
                '--include', include_pattern,
                '--line-numbers',
                '--encoding', 'cl100k',
                '--no-clipboard',
                '--output-file', '/tmp/domain_optimized_context.md'
            ]
            
            # Add domain-specific exclusions
            for exclusion in strategy.get("exclusions", []):
                cmd.extend(['--exclude', exclusion])
            
            # Add domain-specific template if available, fallback to general template
            domain_template_path = Path(__file__).parent / 'templates' / f'{domain}.hbs'
            general_template_path = Path(__file__).parent / 'templates' / 'arda_analysis.hbs'
            
            if domain_template_path.exists():
                cmd.extend(['--template', str(domain_template_path)])
                print(f"📝 Using domain-specific template: {domain}.hbs")
            elif general_template_path.exists():
                cmd.extend(['--template', str(general_template_path)])
                print(f"📝 Using general template: arda_analysis.hbs")
            
            print(f"🔧 Running domain-optimized extraction: {strategy['focus']}")
            start_time = time.time()
            
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
            
            extraction_time = time.time() - start_time
            
            if result.returncode == 0:
                with open('/tmp/domain_optimized_context.md', 'r') as f:
                    context = f.read()
                    
                print(f"✅ Domain-optimized extraction: {len(context):,} characters in {extraction_time:.2f}s")
                print(f"   Focus: {strategy['focus']}")
                print(f"   Performance: {len(context)/extraction_time:.0f} chars/sec")
                
                return context
            else:
                print(f"❌ Domain-optimized extraction failed: {result.stderr}")
                # Fallback to basic extraction
                return self._extract_code_context_with_code2prompt(selected_files)
                
        except Exception as e:
            print(f"❌ Domain optimization error: {e}")
            # Fallback to basic extraction
            return self._extract_code_context_with_code2prompt(selected_files)
    
    def _analyze_code_patterns(self, code_context: str, domain: str, issue: str) -> Dict:
        """Analyze code patterns to extract existing implementations and identify gaps"""
        print(f"🔍 Analyzing code patterns for {domain} domain...")
        
        # Use Nemotron for bulk pattern extraction
        pattern_analysis_prompt = f"""EXPERT CODE PATTERN ANALYSIS

DOMAIN: {domain}
TECHNICAL ISSUE: {issue}

CODE IMPLEMENTATION:
{code_context[:40000]}  # First 40K chars for pattern analysis

ANALYSIS TASKS:

1. EXISTING OBSERVABILITY PATTERNS:
   - Find all #[instrument] annotations with their attributes
   - Identify existing tracing spans and metrics collection
   - Document current logging patterns (info!, warn!, error!)
   - List performance monitoring code

2. ARCHITECTURE ANALYSIS:
   - Map main data flows and critical paths
   - Identify key integration points (external APIs, databases)
   - Document error handling patterns and strategies
   - Find async/await patterns and concurrency handling

3. GAP IDENTIFICATION:
   - Functions missing instrumentation that should have it
   - Critical paths without performance monitoring
   - Error handling gaps or inconsistencies
   - Missing business metrics collection

4. IMPLEMENTATION OPPORTUNITIES:
   - Specific functions needing #[instrument] with suggested span attributes
   - Missing metrics with business justification
   - Performance bottlenecks requiring monitoring
   - Error scenarios needing better handling

OUTPUT FORMAT (JSON):
{{
  "existing_patterns": {{
    "instrumentation": [{{ "function": "function_name", "file": "file.rs", "line": 123, "attributes": "existing #[instrument] attributes" }}],
    "metrics": ["list of existing metrics"],
    "logging": ["existing logging patterns"],
    "error_handling": ["current error handling approaches"]
  }},
  "architecture_insights": {{
    "data_flows": ["main processing flows"],
    "integration_points": ["external service integrations"],
    "critical_paths": ["performance-critical code paths"],
    "concurrency_patterns": ["async/concurrent processing patterns"]
  }},
  "identified_gaps": {{
    "missing_instrumentation": [{{ "function": "function_name", "file": "file.rs", "justification": "why it needs monitoring" }}],
    "missing_metrics": [{{ "metric": "metric_name", "business_value": "cost/performance impact" }}],
    "error_handling_gaps": ["areas needing better error handling"],
    "performance_monitoring_gaps": ["unmonitored critical operations"]
  }},
  "implementation_recommendations": {{
    "high_priority": [{{ "action": "specific change", "file": "file.rs", "implementation": "concrete code example" }}],
    "medium_priority": [{{ "action": "specific change", "file": "file.rs", "implementation": "concrete code example" }}],
    "business_impact": [{{ "change": "technical change", "impact": "business outcome", "estimated_value": "$X/month" }}]
  }}
}}

Focus on actionable, specific recommendations with exact file locations and concrete implementation examples."""

        try:
            # Use bulk processor for pattern analysis
            selected_model = self.models["bulk_processor"]
            
            # Estimate cost
            estimated_input_tokens = len(pattern_analysis_prompt) // 4
            estimated_cost = self._estimate_cost(selected_model, estimated_input_tokens)
            print(f"💰 Pattern analysis cost: ${estimated_cost:.4f} for {selected_model.split('/')[-1]}")
            
            response = openrouter_client.chat.completions.create(
                model=selected_model,
                messages=[{"role": "user", "content": pattern_analysis_prompt}],
                max_tokens=3000,  # More tokens for detailed analysis
                temperature=0.1
            )
            
            analysis_text = response.choices[0].message.content.strip()
            
            # Track usage
            actual_input_tokens = response.usage.prompt_tokens if hasattr(response, 'usage') else estimated_input_tokens
            actual_output_tokens = response.usage.completion_tokens if hasattr(response, 'usage') else len(analysis_text) // 4
            self._track_usage(selected_model, actual_input_tokens, actual_output_tokens)
            
            # Try to parse JSON, fallback to text analysis
            try:
                import json
                pattern_data = json.loads(analysis_text)
                print(f"✅ Successfully parsed pattern analysis JSON")
            except:
                print(f"📝 Using text-based pattern analysis")
                pattern_data = {"analysis_text": analysis_text}
            
            return {
                "domain": domain,
                "patterns": pattern_data,
                "model_used": selected_model,
                "analysis_length": len(analysis_text),
                "success": True
            }
            
        except Exception as e:
            print(f"⚠️ Pattern analysis failed: {e}")
            return {"domain": domain, "patterns": {}, "success": False, "error": str(e)}
    
    def _extract_code_context(self, selected_files: List[str]) -> str:
        """Extract code context using code2prompt"""
        try:
            include_pattern = ",".join(selected_files)
            
            cmd = [
                'code2prompt', self.root_path,
                '--include', include_pattern,
                '--line-numbers',
                '--output-file', '/tmp/dynamic_i2p_context.md'
            ]
            
            result = subprocess.run(cmd, capture_output=True, text=True, cwd=Path(__file__).parent)
            
            if result.returncode == 0:
                with open('/tmp/dynamic_i2p_context.md', 'r') as f:
                    return f.read()
            else:
                print(f"❌ code2prompt failed: {result.stderr}")
                return ""
                
        except Exception as e:
            print(f"❌ Context extraction error: {e}")
            return ""
    
    def _llm_enhance_analysis(self, issue: str, selected_files: List[str], 
                            code_context: str, ast_results: Dict) -> Dict:
        """Hybrid LLM enhancement with smart model selection"""
        
        if not self.enable_llm_enhancement:
            return {"enhanced": False, "analysis": "LLM enhancement disabled"}
        
        print("🧠 Applying hybrid LLM enhancement layer...")
        
        # Determine complexity and select optimal model
        code_size = len(code_context)
        complexity = "High" if len(selected_files) > 2 else "Medium" if len(selected_files) > 1 else "Low"
        selected_model = self._select_optimal_model(issue, code_size, complexity)
        
        # Prepare context for LLM analysis
        codebase_summary = self._summarize_codebase_analysis()
        ast_summary = self._summarize_ast_analysis(ast_results)
        
        # Adjust context size based on model capabilities
        max_context = 120000 if "nemotron" in selected_model else 15000
        focused_context = code_context[:max_context]
        
        # Build model-specific prompt
        if "nemotron" in selected_model:
            enhancement_prompt = self._build_bulk_analysis_prompt(
                issue, codebase_summary, ast_summary, selected_files, focused_context
            )
        else:
            enhancement_prompt = self._build_precision_analysis_prompt(
                issue, codebase_summary, ast_summary, selected_files, focused_context
            )
        
        # Estimate cost before making request
        estimated_input_tokens = len(enhancement_prompt) // 4  # Rough token estimation
        estimated_cost = self._estimate_cost(selected_model, estimated_input_tokens)
        print(f"💰 Estimated cost: ${estimated_cost:.4f} for {selected_model.split('/')[-1]}")
        
        try:
            response = openrouter_client.chat.completions.create(
                model=selected_model,
                messages=[{"role": "user", "content": enhancement_prompt}],
                max_tokens=2000,
                temperature=0.1
            )
            
            llm_analysis = response.choices[0].message.content.strip()
            
            # Track actual usage
            actual_input_tokens = response.usage.prompt_tokens if hasattr(response, 'usage') else estimated_input_tokens
            actual_output_tokens = response.usage.completion_tokens if hasattr(response, 'usage') else len(llm_analysis) // 4
            self._track_usage(selected_model, actual_input_tokens, actual_output_tokens)
            
            return {
                "enhanced": True,
                "analysis": llm_analysis,
                "model_used": selected_model,
                "confidence": 0.9 if "claude" in selected_model else 0.75,
                "context_size": len(focused_context),
                "cost_usd": self._estimate_cost(selected_model, actual_input_tokens, actual_output_tokens)
            }
            
        except Exception as e:
            print(f"⚠️ LLM enhancement failed: {e}")
            return {
                "enhanced": False,
                "analysis": f"LLM enhancement error: {str(e)}",
                "confidence": 0.0,
                "model_used": selected_model
            }
    
    def _llm_enhance_analysis_with_patterns(self, issue: str, selected_files: List[str], 
                                           code_context: str, pattern_analysis: Dict, domain: str) -> Dict:
        """Enhanced LLM analysis that incorporates pattern analysis results"""
        
        if not self.enable_llm_enhancement:
            return {"enhanced": False, "analysis": "LLM enhancement disabled"}
        
        print("🧠 Applying pattern-aware LLM enhancement...")
        
        # Select model based on domain complexity and patterns found
        code_size = len(code_context)
        has_patterns = pattern_analysis.get("success", False)
        complexity = "High" if has_patterns and len(selected_files) > 2 else "Medium"
        
        # For domain-specific analysis, prefer Claude for precision
        if domain in ["zk_observability", "api_security"] or has_patterns:
            selected_model = self.models["code_specialist"]
        else:
            selected_model = self._select_optimal_model(issue, code_size, complexity)
        
        # Build domain-specific prompt with pattern insights
        if has_patterns:
            enhancement_prompt = self._build_pattern_aware_prompt(
                issue, domain, selected_files, code_context, pattern_analysis
            )
        else:
            enhancement_prompt = self._build_precision_analysis_prompt(
                issue, self._summarize_codebase_analysis(), "", selected_files, code_context[:15000]
            )
        
        # Estimate cost
        estimated_input_tokens = len(enhancement_prompt) // 4
        estimated_cost = self._estimate_cost(selected_model, estimated_input_tokens)
        print(f"💰 Enhanced analysis cost: ${estimated_cost:.4f} for {selected_model.split('/')[-1]}")
        
        try:
            response = openrouter_client.chat.completions.create(
                model=selected_model,
                messages=[{"role": "user", "content": enhancement_prompt}],
                max_tokens=2500,
                temperature=0.1
            )
            
            llm_analysis = response.choices[0].message.content.strip()
            
            # Track usage
            actual_input_tokens = response.usage.prompt_tokens if hasattr(response, 'usage') else estimated_input_tokens
            actual_output_tokens = response.usage.completion_tokens if hasattr(response, 'usage') else len(llm_analysis) // 4
            self._track_usage(selected_model, actual_input_tokens, actual_output_tokens)
            
            return {
                "enhanced": True,
                "analysis": llm_analysis,
                "model_used": selected_model,
                "confidence": 0.95 if "claude" in selected_model and has_patterns else 0.85,
                "context_size": len(code_context),
                "pattern_aware": has_patterns,
                "domain": domain,
                "cost_usd": self._estimate_cost(selected_model, actual_input_tokens, actual_output_tokens)
            }
            
        except Exception as e:
            print(f"⚠️ Pattern-aware LLM enhancement failed: {e}")
            return {
                "enhanced": False,
                "analysis": f"Enhanced analysis error: {str(e)}",
                "confidence": 0.0,
                "model_used": selected_model,
                "pattern_aware": False
            }
    
    def _build_pattern_aware_prompt(self, issue: str, domain: str, selected_files: List[str], 
                                  code_context: str, pattern_analysis: Dict) -> str:
        """Build expert-level prompt incorporating pattern analysis insights"""
        
        patterns = pattern_analysis.get("patterns", {})
        existing_patterns = patterns.get("existing_patterns", {})
        identified_gaps = patterns.get("identified_gaps", {})
        recommendations = patterns.get("implementation_recommendations", {})
        
        return f"""SENIOR ARCHITECT: PATTERN-AWARE CODE ANALYSIS

TECHNICAL ISSUE: {issue}
DOMAIN: {domain}
TARGET FILES: {', '.join(selected_files)}

DISCOVERED CODE PATTERNS:
{self._format_pattern_summary(existing_patterns)}

IDENTIFIED GAPS:
{self._format_gap_summary(identified_gaps)}

CURRENT IMPLEMENTATION:
{code_context[:15000]}

EXPERT ANALYSIS REQUIREMENTS:

1. **VALIDATE PATTERN ANALYSIS**: Confirm the accuracy of discovered patterns and gaps
2. **PRIORITIZE RECOMMENDATIONS**: Rank implementation opportunities by business impact
3. **GENERATE CONCRETE IMPLEMENTATIONS**: Provide specific #[instrument] attributes, metric definitions, and code examples
4. **BUSINESS IMPACT ASSESSMENT**: Quantify cost savings, performance improvements, and risk mitigation
5. **IMPLEMENTATION ROADMAP**: Sequential steps with exact file locations and line numbers

FOCUS AREAS:
- Cost optimization (Sindri API efficiency, resource usage)
- Performance monitoring (end-to-end latency, throughput)
- Reliability improvements (error handling, recovery)
- Security enhancements (audit trails, compliance)

Provide expert-level technical recommendations with specific implementation guidance that a senior developer can immediately execute."""

    def _format_pattern_summary(self, patterns: Dict) -> str:
        """Format pattern analysis for LLM consumption"""
        if not patterns:
            return "No existing patterns discovered"
        
        summary = []
        if "instrumentation" in patterns:
            summary.append(f"Instrumentation: {len(patterns['instrumentation'])} functions with #[instrument]")
        if "metrics" in patterns:
            summary.append(f"Metrics: {len(patterns['metrics'])} existing metrics")
        if "logging" in patterns:
            summary.append(f"Logging: {len(patterns['logging'])} logging patterns")
        
        return "\n".join(summary) if summary else "Minimal observability patterns found"
    
    def _format_gap_summary(self, gaps: Dict) -> str:
        """Format gap analysis for LLM consumption"""
        if not gaps:
            return "No specific gaps identified"
        
        summary = []
        if "missing_instrumentation" in gaps:
            summary.append(f"Missing instrumentation: {len(gaps['missing_instrumentation'])} functions need monitoring")
        if "missing_metrics" in gaps:
            summary.append(f"Missing metrics: {len(gaps['missing_metrics'])} business metrics needed")
        if "error_handling_gaps" in gaps:
            summary.append(f"Error handling gaps: {len(gaps['error_handling_gaps'])} areas need improvement")
        
        return "\n".join(summary) if summary else "No critical gaps identified"
    
    def _build_bulk_analysis_prompt(self, issue: str, codebase_summary: str, 
                                  ast_summary: str, selected_files: List[str], 
                                  code_context: str) -> str:
        """Build prompt optimized for bulk processing model (Nemotron)"""
        return f"""BULK CODE ANALYSIS TASK

OBJECTIVE: {issue}

CODEBASE: {codebase_summary}
DISCOVERY: {ast_summary}
FILES: {', '.join(selected_files)}

FULL CODE CONTEXT:
{code_context}

ANALYSIS REQUIREMENTS:
1. PATTERN EXTRACTION: Identify key code patterns related to the issue
2. IMPLEMENTATION GAPS: What's missing or could be improved?
3. CONCRETE STEPS: Specific actionable implementation steps
4. FILE LOCATIONS: Exact files and approximate line ranges to modify
5. IMPACT ASSESSMENT: Potential risks and benefits

Focus on comprehensive coverage and practical implementation guidance."""

    def _build_precision_analysis_prompt(self, issue: str, codebase_summary: str,
                                       ast_summary: str, selected_files: List[str],
                                       code_context: str) -> str:
        """Build prompt optimized for precision analysis (Claude)"""
        return f"""SENIOR ARCHITECT CODE ANALYSIS

TECHNICAL ISSUE: {issue}

CODEBASE CONTEXT: {codebase_summary}
AST DISCOVERY: {ast_summary}
DISCOVERED FILES: {', '.join(selected_files)}

TARGETED CODE IMPLEMENTATION:
{code_context}

DEEP ANALYSIS FRAMEWORK:
1. **Root Cause Assessment**: Is the issue present in discovered code?
2. **Architecture Analysis**: How does current implementation address the domain?
3. **Quality Review**: Identify patterns, anti-patterns, technical debt
4. **Specific Recommendations**: Concrete steps with file/line references
5. **Risk Assessment**: Security, performance, maintainability implications

Provide expert-level technical insights with actionable implementation guidance."""
    
    def _generate_dynamic_prompt(self, issue: str, selected_files: List[str], 
                                code_context: str, ast_results: Dict, 
                                llm_enhancement: Optional[Dict] = None) -> str:
        """Generate optimized prompt using dynamic analysis with optional LLM enhancement"""
        
        codebase_summary = self._summarize_codebase_analysis()
        
        # Extract key implementation details from LLM analysis if available
        implementation_context = ""
        if llm_enhancement and llm_enhancement.get("enhanced"):
            analysis = llm_enhancement.get('analysis', '')
            # Extract specific recommendations and actionable items
            if "## Specific Recommendations" in analysis:
                recommendations_section = analysis.split("## Specific Recommendations")[1].split("## Code Quality Assessment")[0]
                implementation_context = f"""

## Key Implementation Requirements
{recommendations_section.strip()}
"""
        
        prompt = f"""# Project Implementation Request: {issue}

## Executive Summary
**Objective**: {issue}  
**Scope**: {len(selected_files)} files identified for modification  
**Technology Stack**: {', '.join(self.codebase_index.get('language_map', {}).keys())}  
**Estimated Complexity**: {'High' if len(selected_files) > 2 else 'Medium' if len(selected_files) > 1 else 'Low'}

## Project Context
{codebase_summary}

## Implementation Scope

### Target Files
{chr(10).join(f"- **{file}** - {self._get_file_purpose(file)}" for file in selected_files)}

### Current Implementation Analysis
```
{code_context[:12000]}
```
{implementation_context}

## Technical Requirements

### 🎯 Primary Objective
{issue}

### 🔧 Implementation Standards
- **Architecture**: Maintain existing patterns and design principles
- **Compatibility**: Ensure backward compatibility with existing APIs
- **Error Handling**: Implement comprehensive error handling and logging
- **Documentation**: Include inline documentation and update relevant README files
- **Performance**: Optimize for existing performance benchmarks

### 🧪 Quality Assurance Requirements
- **Unit Testing**: Add comprehensive unit tests for new functionality
- **Integration Testing**: Update integration tests to cover new features
- **Regression Testing**: Ensure all existing tests continue to pass
- **Code Review**: Follow existing code review checklist and standards

### 📁 Delivery Artifacts
{chr(10).join(f"- Modified: `{file}`" for file in selected_files)}
- Updated: Unit tests
- Updated: Integration tests  
- Updated: Documentation

## Success Metrics
- ✅ Feature implementation complete and functional
- ✅ All automated tests passing (unit + integration)
- ✅ Code review approval obtained
- ✅ Documentation updated and reviewed
- ✅ No performance regression detected
- ✅ Zero breaking changes to existing functionality

## Implementation Guidelines
You are a senior software engineer assigned to this project. Provide production-ready code that:

1. **Follows Project Conventions**: Match existing code style, naming, and architectural patterns
2. **Implements Robust Error Handling**: Include proper error types, logging, and recovery mechanisms  
3. **Maintains High Code Quality**: Write clean, readable, and maintainable code
4. **Includes Comprehensive Testing**: Cover edge cases and integration scenarios
5. **Documents Changes**: Provide clear comments and update relevant documentation

**Deliverable**: Complete implementation with file-by-file modifications, test updates, and documentation changes.
"""
        
        return prompt
    
    def _generate_expert_prompt(self, issue: str, selected_files: List[str], 
                               code_context: str, pattern_analysis: Dict, 
                               llm_enhancement: Dict, domain: str) -> str:
        """Generate expert-level prompt with pattern analysis and concrete implementation guidance"""
        
        codebase_summary = self._summarize_codebase_analysis()
        
        # Extract business impact for the domain
        business_context = self._get_business_context(domain, pattern_analysis)
        
        # Build implementation section from LLM enhancement
        implementation_guidance = ""
        if llm_enhancement.get("enhanced") and llm_enhancement.get("pattern_aware"):
            implementation_guidance = f"""
## Expert Implementation Analysis
**Model**: {llm_enhancement.get('model_used', 'Unknown').split('/')[-1]}
**Confidence**: {llm_enhancement.get('confidence', 0):.1%}
**Domain**: {domain}

{llm_enhancement.get('analysis', 'No analysis available')}
"""
        
        # Generate domain-specific template
        domain_template = self.domain_templates.get(domain, {})
        
        prompt = f"""# Expert Implementation Guide: {issue}

## Executive Summary
**Objective**: {issue}  
**Domain**: {domain.replace('_', ' ').title()}
**Scope**: {len(selected_files)} target files identified  
**Technology Stack**: {', '.join(self.codebase_index.get('language_map', {}).keys())}  
**Complexity**: High (Expert-level implementation required)

## Business Context
{business_context}

## Implementation Scope

### Target Files & Architecture
{chr(10).join(f"- **{file}** - {self._get_file_purpose(file)}" for file in selected_files)}

### Code Analysis Results
{self._format_pattern_analysis_summary(pattern_analysis)}

### Current Implementation
```
{code_context[:12000]}
```
{implementation_guidance}

## 🤖 AI AGENT EXECUTION SPECIFICATION

### 📋 ENGINEERING TASK BREAKDOWN

#### **TASK IDENTIFIER**: `{domain.upper()}_IMPLEMENTATION_{hash(issue)%10000}`
**Assigned Agent Type**: Code Implementation Specialist
**Estimated Complexity**: High (3-5 hours)
**Dependencies**: Repository access, Rust toolchain, testing framework

#### **ATOMIC IMPLEMENTATION UNITS**

**UNIT 1: DEPENDENCY INTEGRATION**
```toml
# FILE: Cargo.toml (ADD TO [dependencies] SECTION)
metrics = "0.21"
tracing = {{ version = "0.1", features = ["attributes"] }}
thiserror = "1.0"
```
**Validation**: `cargo check --all-features` passes
**Rollback**: Remove dependencies if compilation fails

**UNIT 2: EXACT FUNCTION MODIFICATIONS**
**Execute these precise file changes in order:**

**FILE 1: api/src/batch_processor.rs**
```bash
# STEP 1: Add imports at line 15 (after existing tracing import)
# INSERT AFTER: use tracing::{{debug, error, info, instrument, warn}};
use std::time::Instant;
use metrics::{{counter, histogram}};
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
        transaction_count = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        sindri_cost_usd = tracing::field::Empty,
        success = tracing::field::Empty
    )
)]
```

```rust
# STEP 3: Replace function body starting at line 325
# FIND: async fn process_batch(&self, trigger_type: &str) -> Result<Option<i32>, String> {{
#     info!(
#         "🔄 UNIFIED: Processing batch via {{}} trigger (using unified service)",
#         trigger_type
#     );
# REPLACE WITH:
async fn process_batch(&self, trigger_type: &str) -> Result<Option<i32>, String> {{
    let start_time = Instant::now();
    let span = tracing::Span::current();
    
    // Count trigger attempts
    counter!("batch_processor.triggers.total", 1, "type" => trigger_type);
    
    info!(
        "🔄 UNIFIED: Processing batch via {{}} trigger (using unified service)",
        trigger_type
    );
```

```rust
# STEP 4: Before the final Ok(Some(batch_id)) return statement
# ADD: (Insert before line containing "Ok(Some(batch_id))")
    let duration = start_time.elapsed();
    histogram!("batch_processor.duration_ms", duration.as_millis() as f64);
    
    // Record span fields
    span.record("batch_id", &batch_id);
    span.record("duration_ms", &duration.as_millis());
    span.record("success", &true);
    
    info!(
        batch_id = batch_id,
        duration_ms = duration.as_millis(),
        "✅ Batch processing completed successfully"
    );
```

**FILE 2: api/src/unified_batch_service.rs**
```bash
# STEP 1: Add imports after line 14
# INSERT AFTER: use tracing::{{debug, error, info, instrument, warn}};
use std::time::Instant;
use metrics::{{histogram, counter}};
```

```rust
# STEP 2: Find the create_batch_with_ads function (approximately line 180)
# ADD before existing function signature:
#[instrument(
    skip(self),
    level = "info",
    fields(
        batch_size = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        success = tracing::field::Empty
    )
)]
```

**FILE 3: lib/src/proof.rs**
```bash
# STEP 1: Add imports after existing use statements
# INSERT: 
use std::time::Instant;
use metrics::{{histogram, counter}};
```

```rust
# STEP 2: Find generate_batch_proof function (approximately line 95)
# WRAP the entire function body with timing:
# ADD at function start:
    let start_time = Instant::now();
    counter!("proof_generation.attempts.total", 1, "proof_system" => format!("{{:?}}", request.proof_system));

# ADD before function return:
    let duration = start_time.elapsed();
    histogram!("proof_generation.duration_ms", duration.as_millis() as f64);
```

**UNIT 3: SPECIFIC ERROR HANDLING MODIFICATIONS**

**FILE 1: api/src/batch_processor.rs**
```rust
# STEP 1: Add to imports section (after line 25)
# INSERT:
use thiserror::Error;

# STEP 2: Add error enum before impl BackgroundBatchProcessor (around line 160)
# INSERT:
#[derive(Debug, Error)]
pub enum BatchProcessorError {{
    #[error("Database operation failed: {{source}}")]
    DatabaseError {{ source: String }},
    
    #[error("Sindri API error: {{message}}")]
    SindriError {{ message: String }},
    
    #[error("Proof generation failed: {{reason}}")]
    ProofGenerationError {{ reason: String }},
}}
```

```rust
# STEP 3: Replace specific error return statements
# IN process_batch function, FIND:
        Err(e) => {{
            counter!("batch_processor.errors.total", 1, "trigger" => trigger_type, "error_type" => "proof_generation");
            error!("Batch processing failed: {{}}", e);
            Err(e)
        }}

# REPLACE WITH:
        Err(e) => {{
            let error_type = if e.contains("Sindri") {{
                "sindri_error" 
            }} else if e.contains("database") {{
                "database_error"
            }} else {{
                "proof_generation_error"
            }};
            
            counter!("batch_processor.errors.total", 1, 
                "trigger" => trigger_type, 
                "error_type" => error_type
            );
            
            span.record("success", &false);
            span.record("error_type", error_type);
            
            error!(
                error_type = error_type,
                trigger = trigger_type,
                error_message = %e,
                "❌ Batch processing failed"
            );
            
            Err(format!("Batch processing failed: {{}}", e))
        }}
```

**FILE 2: api/src/unified_batch_service.rs**  
```rust
# STEP 1: Find any string-based error returns (search for 'Err("' patterns)
# EXAMPLE - if found at line ~200:
# FIND: return Err("Failed to create batch".to_string());
# REPLACE: 
return Err("Batch creation failed: ADS integration error".to_string());

# Add error context and metrics
counter!("unified_batch_service.errors.total", 1, "error_type" => "batch_creation");
```

**FILE 3: lib/src/proof.rs**
```rust
# STEP 1: Find ProofError enum (around line 20) and enhance it
# ADD new variants:
#[error("Sindri cost threshold exceeded: estimated ${{cost}}, limit ${{limit}}")]  
CostThresholdExceeded {{ cost: f64, limit: f64 }},

#[error("Proof generation timeout after {{seconds}}s")]
TimeoutError {{ seconds: u64 }},
```

#### **TESTING & VALIDATION PROTOCOL**

**TEST UNIT 1: Compilation Verification**
```bash
# EXECUTE AFTER EACH UNIT IMPLEMENTATION
cargo check --all-features
cargo clippy -- -D warnings
cargo test --lib --bin --test instrumentation
```
**Success Criteria**: Zero compilation errors, zero clippy warnings

**TEST UNIT 2: Performance Regression Testing**
```rust
// ADD: benches/instrumentation_overhead.rs
#[bench]
fn bench_instrumented_function(b: &mut Bencher) {{
    b.iter(|| {{
        // Benchmark instrumented function performance
        // Ensure <2% overhead from baseline
    }});
}}
```

**TEST UNIT 3: Metrics Validation**
```rust
// ADD: tests/integration/metrics_validation.rs
#[tokio::test]
async fn validate_business_metrics_collection() {{
    // Verify metrics are correctly recorded
    // Assert span fields contain expected values
    // Confirm histogram data accuracy
}}
```

#### **🚀 DEPLOYMENT EXECUTION PLAN**

**DEPLOYMENT STEP 1**: Staged Rollout
- [ ] Deploy to development environment first
- [ ] Run automated test suite (100% pass rate required)
- [ ] Performance regression check (<2% overhead)
- [ ] Metrics validation (all KPIs collecting correctly)

**DEPLOYMENT STEP 2**: Production Deployment
```bash
# PRE-DEPLOYMENT VALIDATION
git diff --name-only HEAD~1 HEAD | grep -E "\\.(rs|toml)$"
cargo test --release --all-features
cargo bench --bench instrumentation_overhead

# DEPLOYMENT COMMAND SEQUENCE
git checkout main
git merge feature/{domain}_implementation
cargo build --release
systemctl reload application-service
```

**ROLLBACK PROCEDURE** (If Issues Detected):
```bash
# IMMEDIATE ROLLBACK (< 2 minutes)
git revert HEAD~1
cargo build --release  
systemctl reload application-service

# DEPENDENCY CLEANUP (If Required)
git checkout Cargo.toml && cargo clean && cargo build
```

#### **📊 SUCCESS METRICS (MEASURABLE)**

**Business KPIs**: {self._format_domain_kpis(domain_template.get("business_metrics", []))}

**Technical Validation Checklist**:
- [ ] All target functions have `#[instrument]` attributes
- [ ] Business metrics appear in telemetry dashboard  
- [ ] Error handling uses structured error types
- [ ] Performance overhead <2% measured via benchmarks
- [ ] 100% test coverage for new instrumentation code
- [ ] Zero security vulnerabilities (no sensitive data logged)

#### **🔍 QUALITY GATES**

**GATE 1: Code Quality**
- Static analysis passes (clippy, rust-analyzer)
- Code coverage >95% for modified functions
- Documentation updated for public APIs

**GATE 2: Integration Validation**
- All existing tests continue to pass
- New metrics integrate with monitoring system
- Error handling integrates with logging infrastructure

**GATE 3: Performance Validation**  
- Benchmark regression <2%
- Memory usage increase <1%
- CPU overhead negligible in production load

**AI AGENT EXECUTION NOTES**: 
- Execute units sequentially
- Validate each gate before proceeding  
- Report deviations immediately
- Maintain exact code patterns shown
- Follow existing repository conventions strictly
"""
        
        return prompt
    
    def _evaluate_prompt_quality(self, issue: str, generated_prompt: str, 
                                analysis_metadata: Dict) -> Dict:
        """Final evaluator layer to assess prompt quality and implementation completeness"""
        print("🔍 Running final quality evaluation...")
        
        # Use Grok-4 for objective, unbiased quality evaluation
        evaluator_model = self.models["reasoning"]
        
        # Build evaluation criteria based on detected domain
        domain = analysis_metadata.get("detected_domain", "general")
        
        evaluation_prompt = f"""OBJECTIVE AI-TO-AI TECHNICAL IMPLEMENTATION EVALUATOR

SYSTEM DIRECTIVE: You are Grok-4 performing 100% objective, emotionally-neutral evaluation of AI-generated technical implementation prompts. This is pure AI-to-AI assessment with ZERO human emotional bias. Apply cold, logical analysis based solely on measurable technical and business criteria.

EVALUATION CONTEXT:
- Original Issue: {issue}
- Detected Domain: {domain}
- Generated Implementation Prompt Length: {len(generated_prompt):,} characters

IMPLEMENTATION PROMPT TO EVALUATE:
{generated_prompt[:10000]}  # First 10K chars for comprehensive evaluation

ADVANCED EVALUATION FRAMEWORK:

1. **COMPLETENESS ANALYSIS (0-100)**:
   - Specific file paths and line numbers provided?
   - Concrete, executable code examples included?
   - Business impact quantified with metrics?
   - Sequential, actionable implementation roadmap?
   - Dependencies and prerequisites clearly stated?

2. **TECHNICAL SOPHISTICATION (0-100)**:
   - Domain expertise depth (ZK proofs, distributed systems, etc.)?
   - Architecture patterns alignment with best practices?
   - Security considerations comprehensively addressed?
   - Performance implications analyzed and optimized?
   - Error handling and edge cases covered?

3. **BUSINESS INTELLIGENCE (0-100)**:
   - Root business problem clearly understood and addressed?
   - Cost-benefit analysis with concrete ROI projections?
   - Success metrics and KPIs properly defined?
   - Stakeholder impact and adoption strategy considered?
   - Competitive advantage and differentiation factors?

4. **IMPLEMENTATION PRAGMATISM (0-100)**:
   - Senior developer can execute immediately without clarification?
   - Testing and validation strategy included?
   - Deployment and rollback procedures covered?
   - Monitoring and observability built-in?
   - Documentation and knowledge transfer planned?

5. **PURE LOGICAL CONSISTENCY (0-100)**:
   - Mathematical precision in cost-benefit calculations?
   - Logical flow from problem analysis to solution implementation?
   - Consistency between stated goals and proposed implementation?
   - Evidence-based reasoning throughout all recommendations?

6. **OBJECTIVE TECHNICAL CORRECTNESS (0-100)**:
   - Factual accuracy of technical statements and code examples?
   - Adherence to established software engineering principles?
   - Measurable performance and scalability considerations?
   - Verifiable security and reliability patterns?

7. **SYSTEMATIC COMPLETENESS ANALYSIS**:
   - Quantifiable gaps in implementation coverage?
   - Missing critical dependencies or prerequisites?
   - Unaddressed failure modes or edge cases?
   - Incomplete testing or validation strategies?

REQUIRED OUTPUT (Pure Objective JSON):
{{
  "overall_quality_score": 88,
  "objective_dimensions": {{
    "completeness": 92,
    "technical_sophistication": 89, 
    "business_intelligence": 85,
    "implementation_pragmatism": 87,
    "logical_consistency": 90,
    "technical_correctness": 94
  }},
  "measurable_gaps": {{
    "missing_file_paths": 0,
    "missing_code_examples": 2,
    "missing_cost_calculations": 1,
    "missing_testing_strategy": 0,
    "missing_error_handling": 3
  }},
  "technical_accuracy_verification": {{
    "factual_errors_detected": 0,
    "outdated_patterns_used": 1,
    "security_vulnerabilities": 0,
    "performance_oversights": 2
  }},
  "implementation_feasibility": {{
    "executable_immediately": true/false,
    "requires_clarification": true/false,
    "missing_dependencies": ["specific dependencies not mentioned"],
    "estimated_implementation_hours": 24
  }},
  "mathematical_precision": {{
    "cost_calculations_accurate": true/false,
    "performance_metrics_quantified": true/false,
    "roi_projections_realistic": true/false,
    "resource_estimates_precise": true/false
  }},
  "critical_deficiencies": [
    "Ranked list of most serious implementation gaps that could cause project failure"
  ],
  "quality_rating": "exceptional|excellent|good|acceptable|inadequate",
  "confidence_score": 0.98,
  "evaluation_rationale": "Purely logical reasoning for scores based on measurable criteria"
}}

EVALUATION MANDATE: Provide brutally honest, mathematically precise assessment. No diplomatic language. No emotional considerations. Pure technical and business logic only. If implementation gaps exist, quantify them exactly. If technical errors are present, identify them specifically. This is AI-to-AI evaluation - apply maximum analytical precision."""
        
        try:
            # Estimate cost for evaluation
            estimated_input_tokens = len(evaluation_prompt) // 4
            estimated_cost = self._estimate_cost(evaluator_model, estimated_input_tokens, 800)
            print(f"💰 Evaluation cost: ${estimated_cost:.4f} for {evaluator_model.split('/')[-1]}")
            
            response = openrouter_client.chat.completions.create(
                model=evaluator_model,
                messages=[{"role": "user", "content": evaluation_prompt}],
                max_tokens=1500,
                temperature=0.0  # Zero temperature for consistent evaluation
            )
            
            evaluation_text = response.choices[0].message.content.strip()
            
            # Track usage
            actual_input_tokens = response.usage.prompt_tokens if hasattr(response, 'usage') else estimated_input_tokens
            actual_output_tokens = response.usage.completion_tokens if hasattr(response, 'usage') else len(evaluation_text) // 4
            self._track_usage(evaluator_model, actual_input_tokens, actual_output_tokens)
            
            # Try to parse JSON evaluation
            try:
                import json
                # Extract JSON from response (handle potential markdown formatting)
                json_start = evaluation_text.find('{')
                json_end = evaluation_text.rfind('}') + 1
                if json_start != -1 and json_end > json_start:
                    json_content = evaluation_text[json_start:json_end]
                    evaluation_data = json.loads(json_content)
                else:
                    raise ValueError("No JSON found in response")
                
                print(f"✅ Quality evaluation complete:")
                overall_score = evaluation_data.get('overall_quality_score', 0)
                rating = evaluation_data.get('quality_rating', 'unknown')
                confidence = evaluation_data.get('confidence_score', evaluation_data.get('confidence', 0))
                
                print(f"   Overall Score: {overall_score}/100")
                print(f"   Rating: {rating.upper()}")
                print(f"   Confidence: {confidence:.1%}")
                
                # Show objective dimension scores (Pure AI evaluation)
                objective_dims = evaluation_data.get('objective_dimensions', {})
                if objective_dims:
                    completeness = objective_dims.get('completeness', 0)
                    technical = objective_dims.get('technical_sophistication', 0)  
                    business = objective_dims.get('business_intelligence', 0)
                    pragmatism = objective_dims.get('implementation_pragmatism', 0)
                    logical = objective_dims.get('logical_consistency', 0)
                    correctness = objective_dims.get('technical_correctness', 0)
                    print(f"   Objective Dimensions: Complete:{completeness}, Tech:{technical}, Business:{business}, Pragmatic:{pragmatism}, Logic:{logical}, Correct:{correctness}")
                
                # Show measurable gaps (Pure quantitative analysis)
                gaps = evaluation_data.get('measurable_gaps', {})
                if gaps:
                    total_gaps = sum(gaps.values()) if isinstance(list(gaps.values())[0], int) else len(gaps)
                    print(f"   Quantified Gaps: {total_gaps} measurable deficiencies detected")
                
                # Show technical accuracy verification
                tech_accuracy = evaluation_data.get('technical_accuracy_verification', {})
                if tech_accuracy:
                    errors = tech_accuracy.get('factual_errors_detected', 0)
                    vulnerabilities = tech_accuracy.get('security_vulnerabilities', 0)
                    oversights = tech_accuracy.get('performance_oversights', 0)
                    if errors + vulnerabilities + oversights > 0:
                        print(f"   Technical Issues: {errors} errors, {vulnerabilities} security issues, {oversights} performance gaps")
                    else:
                        print(f"   Technical Verification: Clean - no factual errors or security vulnerabilities detected")
                
                # Show implementation feasibility
                feasibility = evaluation_data.get('implementation_feasibility', {})
                if feasibility:
                    executable = feasibility.get('executable_immediately', False)
                    hours = feasibility.get('estimated_implementation_hours', 0)
                    print(f"   Implementation: {'Immediately executable' if executable else 'Requires preparation'} - Est. {hours}h effort")
                
                return {
                    "evaluated": True,
                    "evaluation": evaluation_data,
                    "evaluator_model": evaluator_model,
                    "evaluation_cost": self._estimate_cost(evaluator_model, actual_input_tokens, actual_output_tokens),
                    "raw_evaluation": evaluation_text
                }
                
            except (json.JSONDecodeError, ValueError) as e:
                print(f"📝 Using text-based evaluation (JSON parse failed: {e})")
                
                # Extract key metrics from text if JSON parsing fails
                overall_score = 85  # Default assumption of good quality
                if "overall_quality_score" in evaluation_text:
                    try:
                        import re
                        score_match = re.search(r"overall_quality_score[\"']?\s*:\s*(\d+)", evaluation_text)
                        if score_match:
                            overall_score = int(score_match.group(1))
                    except:
                        pass
                
                return {
                    "evaluated": True,
                    "evaluation": {
                        "overall_quality_score": overall_score,
                        "quality_rating": "good" if overall_score >= 80 else "acceptable",
                        "raw_text": evaluation_text
                    },
                    "evaluator_model": evaluator_model,
                    "evaluation_cost": self._estimate_cost(evaluator_model, actual_input_tokens, actual_output_tokens)
                }
                
        except Exception as e:
            print(f"⚠️ Quality evaluation failed: {e}")
            return {
                "evaluated": False,
                "evaluation": {"error": str(e)},
                "evaluator_model": evaluator_model
            }
    
    def _get_business_context(self, domain: str, pattern_analysis: Dict) -> str:
        """Generate business context based on domain and analysis"""
        
        domain_mapping = {
            "zk_observability": "zero-knowledge proof pipeline",
            "database_optimization": "database operations", 
            "api_security": "API security and authentication",
            "error_handling": "system reliability and error management"
        }
        
        business_area = domain_mapping.get(domain, domain.replace('_', ' '))
        
        # Get business impact from mapping
        business_data = self.business_impact_mapping.get(domain.replace('_observability', '').replace('_optimization', '').replace('_security', ''), {})
        
        cost_drivers = business_data.get("cost_drivers", ["operational overhead", "system downtime"])
        performance_kpis = business_data.get("performance_kpis", ["response time", "reliability"])
        typical_costs = business_data.get("typical_costs", {})
        
        cost_context = ""
        if typical_costs:
            cost_examples = [f"{k.replace('_', ' ')}: ${v:,}/month" for k, v in typical_costs.items()]
            cost_context = f"**Typical Cost Impact**: {', '.join(cost_examples)}"
        
        return f"""**Business Area**: {business_area.title()}
**Cost Drivers**: {', '.join(cost_drivers)}
**Performance KPIs**: {', '.join(performance_kpis)}
{cost_context}
**ROI Opportunity**: Implementing comprehensive observability typically reduces operational costs by 15-25% and improves system reliability by 2-3x"""

    def _format_pattern_analysis_summary(self, pattern_analysis: Dict) -> str:
        """Format pattern analysis results for executive summary"""
        if not pattern_analysis.get("success"):
            return "**Pattern Analysis**: Not available (basic analysis mode)"
        
        patterns = pattern_analysis.get("patterns", {})
        
        if isinstance(patterns, dict) and "existing_patterns" in patterns:
            existing = patterns.get("existing_patterns", {})
            gaps = patterns.get("identified_gaps", {})
            
            instrumentation_count = len(existing.get("instrumentation", []))
            gap_count = len(gaps.get("missing_instrumentation", []))
            
            return f"""**Pattern Analysis**: {instrumentation_count} existing instrumented functions, {gap_count} functions need instrumentation
**Architecture Insights**: {len(patterns.get("architecture_insights", {}).get("critical_paths", []))} critical paths identified
**Implementation Opportunities**: {len(patterns.get("implementation_recommendations", {}).get("high_priority", []))} high-priority recommendations"""
        
        return f"""**Pattern Analysis**: Completed ({pattern_analysis.get('analysis_length', 0):,} chars analyzed)
**Model Used**: {pattern_analysis.get('model_used', 'Unknown').split('/')[-1]}"""

    def _format_domain_kpis(self, business_metrics: List[str]) -> str:
        """Format domain-specific KPIs"""
        if not business_metrics:
            return "- Response time and throughput\n- Error rates and reliability\n- Resource utilization and costs"
        
        return chr(10).join(f"- {metric.replace('_', ' ').title()}" for metric in business_metrics)
    
    def _get_file_purpose(self, file_path: str) -> str:
        """Get a brief description of the file's purpose"""
        path_parts = file_path.split('/')
        
        if 'operations.rs' in file_path:
            return "Core operation types and nullifier generation"
        elif 'error.rs' in file_path:
            return "Error handling and type definitions"
        elif 'transaction_mapper.rs' in file_path:
            return "Transaction mapping and processing logic"
        elif 'lib.rs' in file_path:
            return "Module declarations and public exports"
        elif 'main.rs' in file_path:
            return "Application entry point"
        elif 'mod.rs' in file_path:
            return "Module definition and organization"
        elif any(part in ['api', 'server'] for part in path_parts):
            return "API server and endpoint handling"
        elif any(part in ['db', 'database'] for part in path_parts):
            return "Database operations and data management"
        elif any(part in ['cli', 'cmd'] for part in path_parts):
            return "Command-line interface and utilities"
        elif any(part in ['contract', 'solidity'] for part in path_parts):
            return "Smart contract implementation"
        else:
            return "Core functionality and business logic"
    
    def _summarize_codebase_analysis(self) -> str:
        """Generate summary of codebase structure analysis"""
        languages = self.codebase_index.get('language_map', {})
        frameworks = self.codebase_index.get('framework_detection', {})
        directories = self.codebase_index.get('directory_purposes', {})
        size_metrics = self.codebase_index.get('size_metrics', {})
        
        active_frameworks = [fw for fw, active in frameworks.items() if active]
        
        return f"""
**Languages**: {', '.join(f"{lang}({count})" for lang, count in languages.items())}
**Frameworks**: {', '.join(active_frameworks) if active_frameworks else 'None detected'}
**Architecture**: {len(directories)} functional directories detected
**Size**: {size_metrics.get('total_files', 0)} files, {size_metrics.get('total_size_bytes', 0):,} bytes
**Key Directories**: {', '.join(f"{path}({purpose})" for path, purpose in list(directories.items())[:5])}
""".strip()
    
    def _summarize_ast_analysis(self, ast_results: Dict) -> str:
        """Generate summary of AST query results"""
        if not ast_results:
            return "**AST Analysis**: No AST queries executed (fallback mode)"
        
        total_queries = len(ast_results)
        total_matches = sum(len(matches) for matches in ast_results.values())
        successful_queries = sum(1 for matches in ast_results.values() if matches)
        
        return f"""
**AST Queries Executed**: {total_queries}
**Successful Queries**: {successful_queries}/{total_queries}
**Total Pattern Matches**: {total_matches}
**Discovery Method**: Structure-guided AST pattern matching
""".strip()

# ============================================================================
# MAIN FUNCTION - USES DYNAMIC I2P SYSTEM
# ============================================================================

def main():
    """Dynamic I2P system with universal AST-powered discovery"""
    
    # Parse command line arguments
    import argparse
    parser = argparse.ArgumentParser(description="Dynamic I2P Analysis System")
    parser.add_argument("issue", nargs="*", default=["Add database connection pooling to prevent timeouts"], 
                       help="Issue to analyze")
    parser.add_argument("--llm", action="store_true", help="Enable LLM enhancement layer")
    
    args = parser.parse_args()
    issue = " ".join(args.issue)
    
    # Use new Dynamic I2P system
    dynamic_i2p = DynamicI2P()
    
    # Enable LLM enhancement if requested
    if args.llm:
        dynamic_i2p.enable_llm_enhancement = True
        print("🧠 LLM Enhancement Layer: ENABLED")
    
    result = dynamic_i2p.analyze_issue(issue)
    
    if result.get("success"):
        print(f"\n✅ Dynamic I2P Analysis Complete!")
        print(f"   Method: {result.get('method', 'unknown')}")
        print(f"   Files: {result['selected_files']}")
        print(f"   Discovery: {result.get('file_discovery_method', 'unknown')}")
        print(f"   Domain: {result.get('detected_domain', 'general')}")
        print(f"   Context: {result['code_context_length']:,} chars")
        
        # Show pattern analysis info if available
        pattern_info = result.get('pattern_analysis', {})
        if pattern_info.get('success'):
            print(f"   🔍 Pattern Analysis: Success ({pattern_info.get('analysis_length', 0):,} chars)")
        
        # Show LLM enhancement info if available
        llm_info = result.get('llm_enhancement', {})
        if llm_info.get('enhanced'):
            model_name = llm_info.get('model_used', 'Unknown').split('/')[-1]
            confidence = llm_info.get('confidence', 0)
            context_size = llm_info.get('context_size', 0)
            cost = llm_info.get('cost_usd', 0)
            print(f"   🧠 LLM Enhancement: {model_name} (confidence: {confidence:.1%}, context: {context_size:,} chars, cost: ${cost:.4f})")
        else:
            print(f"   🧠 LLM Enhancement: Disabled")
        
        # Show quality evaluation info (Pure AI-to-AI objective assessment)
        eval_info = result.get('quality_evaluation', {})
        if eval_info.get('evaluated'):
            evaluation = eval_info.get('evaluation', {})
            overall_score = evaluation.get('overall_quality_score', 0)
            quality_rating = evaluation.get('quality_rating', 'unknown')
            eval_cost = eval_info.get('evaluation_cost', 0)
            evaluator_model = eval_info.get('evaluator_model', 'unknown').split('/')[-1]
            
            print(f"   📋 AI Evaluation ({evaluator_model}): {overall_score}/100 ({quality_rating.upper()}, cost: ${eval_cost:.4f})")
            
            # Show objective dimensions if available
            objective_dims = evaluation.get('objective_dimensions', {})
            if objective_dims:
                logical = objective_dims.get('logical_consistency', 0)
                correctness = objective_dims.get('technical_correctness', 0) 
                print(f"      Logic: {logical}/100, Correctness: {correctness}/100")
            
            # Show quantified gaps and technical issues
            gaps = evaluation.get('measurable_gaps', {})
            tech_issues = evaluation.get('technical_accuracy_verification', {})
            if gaps or tech_issues:
                total_gaps = sum(gaps.values()) if gaps and isinstance(list(gaps.values())[0], int) else 0
                errors = tech_issues.get('factual_errors_detected', 0) if tech_issues else 0
                if total_gaps > 0 or errors > 0:
                    print(f"      Issues: {total_gaps} gaps, {errors} technical errors detected")
        else:
            print(f"   📋 AI Evaluation: Disabled or failed")
        
        # Show cost tracking
        cost_info = result.get('cost_tracking', {})
        if cost_info.get('total_cost_usd', 0) > 0:
            print(f"   💰 Total Cost: ${cost_info['total_cost_usd']:.4f} ({cost_info['total_input_tokens']:,} in + {cost_info['total_output_tokens']:,} out tokens)")
            
        print(f"   ⏱️ Time: {result['processing_time']:.1f}s")
        
        # Display codebase analysis summary
        if 'codebase_analysis' in result:
            print(f"\n📊 Codebase Analysis:")
            print(result['codebase_analysis'])
        
        # Save prompt
        output_file = Path("i2p_dynamic_prompt.md")
        with open(output_file, 'w') as f:
            f.write(result['optimized_prompt'])
        
        print(f"💾 Prompt saved to: {output_file}")
    else:
        print(f"❌ Analysis failed: {result.get('error')}")


if __name__ == "__main__":
    main()