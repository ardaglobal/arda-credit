# ACT - Local GitHub Actions Testing

This project uses [act](https://nektosact.com/usage/index.html) to run GitHub Actions workflows locally for testing.

## Setup

1. **Install act** (see [official docs](https://nektosact.com/usage/index.html))

2. **Copy secrets file and update values:**
   ```bash
   cp .act_secrets.example .act_secrets
   # Edit .act_secrets with your tokens
   ```

3. **Architecture configuration:** 
   - **M1/M2 Macs**: Use existing `.actrc` configuration
   - **Intel machines**: Comment out `--container-architecture=linux/arm64` in `.actrc`

## Required Secrets

Update `.act_secrets` with your actual tokens:

```bash
GITHUB_TOKEN=ghp_your_token_here
SP1_PAT=ghp_your_sp1_token_here
SINDRI_API_KEY=sindri_your_key_here
DOCKERHUB_USERNAME=<username>
DOCKERHUB_TOKEN=<token>
```

## Workflow Modifications for Local Testing

**Replace self-hosted runners with standard runners for local testing:**

**Original (GitHub):**
```yaml
runs-on:
  - codebuild-arda-credit-${{ github.run_id }}-${{ github.run_attempt }}
  - image=ubuntu-7.0
  - instance-size=small
```

**For local testing:**
```yaml
runs-on: ubuntu-latest
```

**Remember to change back after testing!**

## Usage

```bash
# Test specific workflows
act --secret-file .act_secrets -W '.github/workflows/test.yml'
act --secret-file .act_secrets -W '.github/workflows/coverage.yml'  
act --secret-file .act_secrets -W '.github/workflows/sindri.yml'

# List available workflows
act -l

# Run with verbose output for debugging
act -v
```