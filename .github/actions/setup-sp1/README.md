# Setup SP1 Toolchain Action

A reusable composite GitHub Action that installs and caches the SP1 (Succinct Proof 1) toolchain for faster CI/CD builds.

## Features

- ✅ **Automatic caching**: SP1 toolchain is cached between workflow runs to dramatically reduce installation time
- 🚀 **Fast execution**: Cache hits provide near-instantaneous SP1 setup
- 🔧 **Configurable verification**: Optional installation verification via `cargo-prove --version`
- 📦 **Smart cache keys**: Uses SP1 URL hash for cache invalidation when needed
- 🛣️ **PATH management**: Automatically adds SP1 tools to the GitHub Actions PATH

## Usage

### Basic Usage

```yaml
steps:
  - name: Checkout code
    uses: actions/checkout@v5

  - name: Install SP1 toolchain
    uses: ./.github/actions/setup-sp1
```

### With Custom Configuration

```yaml
steps:
  - name: Install SP1 toolchain
    uses: ./.github/actions/setup-sp1
    with:
      verify-installation: 'true'       # Verify installation (default: 'true')
      cache-key-suffix: 'v1.0'         # Optional cache key suffix
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `verify-installation` | Whether to verify installation by running `cargo-prove --version` | No | `'true'` |
| `cache-key-suffix` | Additional suffix for cache key (useful for different SP1 versions) | No | `''` |

## Outputs

This action automatically:
- Adds `$HOME/.sp1/bin` to the GitHub Actions PATH
- Caches the SP1 installation for subsequent workflow runs
- Provides console output indicating whether SP1 was loaded from cache or freshly installed

## Performance Benefits

**Before (without caching):**
- SP1 installation time: ~2-3 minutes per workflow
- Network bandwidth: Downloads SP1 tools every time
- Resource usage: High CPU/memory during installation

**After (with caching):**
- First run: ~2-3 minutes (same as before, but cached)
- Subsequent runs: ~5-10 seconds (cache hit)
- Network bandwidth: Minimal (only cache operations)
- Resource usage: Minimal overhead

## Cache Strategy

The action uses a sophisticated caching strategy:

1. **Cache Key**: Based on SP1 installation URL hash + optional suffix
2. **Cache Path**: `~/.sp1` (entire SP1 installation directory)
3. **Fallback Keys**: Provides fallback cache keys for maximum cache hit ratio
4. **Invalidation**: Cache automatically invalidates if SP1 installation changes

## Migration from Manual Installation

### Before

```yaml
- name: Install SP1 toolchain
  run: |
    curl -L https://sp1.succinct.xyz | bash
    export PATH="$HOME/.sp1/bin:$PATH"
    sp1up
    echo "$HOME/.sp1/bin" >> $GITHUB_PATH
```

### After

```yaml
- name: Install SP1 toolchain
  uses: ./.github/actions/setup-sp1
```

## Workflows Updated

This action is currently used in the following workflows:
- `coverage.yml` - Code coverage generation
- `test.yml` - Test execution
- `format.yml` - Code formatting and linting
- `sindri.yml` - Sindri circuit deployment
- `prove.yml` - SP1 program building and execution

## Troubleshooting

### Cache Issues

If you encounter SP1-related issues, you can clear the cache by:
1. Using a different `cache-key-suffix` in your workflow
2. Manually clearing the GitHub Actions cache through the repository settings

### PATH Issues

The action automatically manages the PATH. After the action runs, you can use SP1 tools directly:

```yaml
- name: Use SP1 tools
  run: |
    cargo-prove prove --version
    # No need for ~/.sp1/bin/cargo-prove
```

## Contributing

To modify this action:
1. Update the action.yml file
2. Test with a sample workflow
3. Update this README if needed
4. Update workflows that use this action if breaking changes are introduced