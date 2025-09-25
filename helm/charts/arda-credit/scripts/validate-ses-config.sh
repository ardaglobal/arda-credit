#!/bin/bash

# Validate AWS/SES Configuration for Arda Credit HELM Chart
# This script validates that all required environment variables and secrets are properly configured

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-arda-credit}"
SECRET_NAME="arda-credit-secrets"
CONFIGMAP_NAME="arda-credit-config"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Validating AWS/SES Configuration for Arda Credit${NC}"
echo ""

# Track validation results
validation_passed=true
warnings=()

# Function to check if a secret key exists and is not empty
check_secret_key() {
    local key="$1"
    local description="$2"
    
    if kubectl get secret "$SECRET_NAME" -n "$NAMESPACE" -o jsonpath="{.data.$key}" 2>/dev/null | base64 -d | grep -q .; then
        echo -e "  ✅ $description ($key)"
        return 0
    else
        echo -e "  ❌ $description ($key) - Missing or empty"
        validation_passed=false
        return 1
    fi
}

# Function to check if a configmap key exists and is not empty
check_configmap_key() {
    local key="$1"
    local description="$2"
    local required="${3:-true}"
    
    if kubectl get configmap "$CONFIGMAP_NAME" -n "$NAMESPACE" -o jsonpath="{.data.$key}" 2>/dev/null | grep -q .; then
        echo -e "  ✅ $description ($key)"
        return 0
    else
        if [[ "$required" == "true" ]]; then
            echo -e "  ❌ $description ($key) - Missing or empty"
            validation_passed=false
            return 1
        else
            echo -e "  ⚠️  $description ($key) - Optional, not set"
            warnings+=("$description ($key) is not configured")
            return 0
        fi
    fi
}

# Check if namespace exists
echo -e "${YELLOW}📦 Checking namespace: $NAMESPACE${NC}"
if kubectl get namespace "$NAMESPACE" >/dev/null 2>&1; then
    echo -e "  ✅ Namespace exists"
else
    echo -e "  ❌ Namespace does not exist"
    echo -e "${RED}Please create the namespace first: kubectl create namespace $NAMESPACE${NC}"
    exit 1
fi

# Check if secrets exist
echo ""
echo -e "${YELLOW}🔐 Checking application secrets...${NC}"
if kubectl get secret "$SECRET_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    echo -e "  ✅ Secret $SECRET_NAME exists"
    
    # Check required secret keys
    echo ""
    echo -e "${BLUE}Validating AWS/SES credentials:${NC}"
    check_secret_key "aws-access-key-id" "AWS Access Key ID"
    check_secret_key "aws-secret-access-key" "AWS Secret Access Key"
    check_secret_key "email-from" "Email From Address"
    check_secret_key "email-reply-to" "Email Reply-To Address"
    
    echo ""
    echo -e "${BLUE}Validating other application secrets:${NC}"
    check_secret_key "sindri-api-key" "Sindri API Key"
    check_secret_key "ethereum-rpc-url" "Ethereum RPC URL"
    check_secret_key "ethereum-contract-address" "Ethereum Contract Address"
    check_secret_key "ethereum-wallet-private-key" "Ethereum Wallet Private Key"
    check_secret_key "ethereum-deployer-address" "Ethereum Deployer Address"
    
else
    echo -e "  ❌ Secret $SECRET_NAME does not exist"
    validation_passed=false
    echo ""
    echo -e "${YELLOW}Create the secret with:${NC}"
    echo "kubectl create secret generic $SECRET_NAME \\"
    echo "  --from-literal=aws-access-key-id=\$AWS_ACCESS_KEY_ID \\"
    echo "  --from-literal=aws-secret-access-key=\$AWS_SECRET_ACCESS_KEY \\"
    echo "  --from-literal=email-from=\$EMAIL_FROM \\"
    echo "  --from-literal=email-reply-to=\$EMAIL_REPLY_TO \\"
    echo "  --from-literal=sindri-api-key=\$SINDRI_API_KEY \\"
    echo "  --from-literal=ethereum-rpc-url=\$ETHEREUM_RPC_URL \\"
    echo "  --from-literal=ethereum-contract-address=\$ETHEREUM_CONTRACT_ADDRESS \\"
    echo "  --from-literal=ethereum-wallet-private-key=\$ETHEREUM_WALLET_PRIVATE_KEY \\"
    echo "  --from-literal=ethereum-deployer-address=\$ETHEREUM_DEPLOYER_ADDRESS \\"
    echo "  --namespace=$NAMESPACE"
fi

# Check if configmap exists
echo ""
echo -e "${YELLOW}⚙️  Checking configuration...${NC}"
if kubectl get configmap "$CONFIGMAP_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    echo -e "  ✅ ConfigMap $CONFIGMAP_NAME exists"
    
    # Check required configmap keys
    echo ""
    echo -e "${BLUE}Validating AWS/SES configuration:${NC}"
    check_configmap_key "SES_REGION" "AWS SES Region"
    check_configmap_key "MAGIC_BASE_URL" "Magic Link Base URL"
    check_configmap_key "EMAIL_ENABLED" "Email Service Enabled"
    check_configmap_key "EMAIL_DEBUG_MODE" "Email Debug Mode"
    check_configmap_key "SES_CONFIGURATION_SET" "SES Configuration Set" "false"
    
else
    echo -e "  ❌ ConfigMap $CONFIGMAP_NAME does not exist"
    validation_passed=false
fi

# Check GHCR secret
echo ""
echo -e "${YELLOW}🐳 Checking container registry access...${NC}"
if kubectl get secret "ghcr-secret" -n "$NAMESPACE" >/dev/null 2>&1; then
    echo -e "  ✅ GHCR secret exists"
else
    echo -e "  ❌ GHCR secret does not exist"
    validation_passed=false
    echo ""
    echo -e "${YELLOW}Create the GHCR secret with:${NC}"
    echo "kubectl create secret docker-registry ghcr-secret \\"
    echo "  --docker-server=ghcr.io \\"
    echo "  --docker-username=arda-bot \\"
    echo "  --docker-password=\$GITHUB_ORG_PAT \\"
    echo "  --namespace=$NAMESPACE"
fi

# Validate email addresses format
echo ""
echo -e "${YELLOW}📧 Validating email configuration...${NC}"
if kubectl get secret "$SECRET_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    email_from=$(kubectl get secret "$SECRET_NAME" -n "$NAMESPACE" -o jsonpath="{.data.email-from}" 2>/dev/null | base64 -d || echo "")
    email_reply_to=$(kubectl get secret "$SECRET_NAME" -n "$NAMESPACE" -o jsonpath="{.data.email-reply-to}" 2>/dev/null | base64 -d || echo "")
    
    # Basic email validation
    if [[ -n "$email_from" ]]; then
        if [[ "$email_from" =~ ^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$ ]]; then
            echo -e "  ✅ Email From format is valid: $email_from"
        else
            echo -e "  ❌ Email From format is invalid: $email_from"
            validation_passed=false
        fi
    fi
    
    if [[ -n "$email_reply_to" ]]; then
        if [[ "$email_reply_to" =~ ^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$ ]]; then
            echo -e "  ✅ Email Reply-To format is valid: $email_reply_to"
        else
            echo -e "  ❌ Email Reply-To format is invalid: $email_reply_to"
            validation_passed=false
        fi
    fi
fi

# Check AWS SES region validity
echo ""
echo -e "${YELLOW}🌍 Validating AWS configuration...${NC}"
if kubectl get configmap "$CONFIGMAP_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    ses_region=$(kubectl get configmap "$CONFIGMAP_NAME" -n "$NAMESPACE" -o jsonpath="{.data.SES_REGION}" 2>/dev/null || echo "")
    
    # Valid AWS SES regions
    valid_regions=("us-east-1" "us-east-2" "us-west-1" "us-west-2" "ap-south-1" "ap-northeast-2" "ap-southeast-1" "ap-southeast-2" "ap-northeast-1" "ca-central-1" "eu-central-1" "eu-west-1" "eu-west-2" "eu-west-3" "eu-north-1" "sa-east-1")
    
    if [[ " ${valid_regions[*]} " =~ " ${ses_region} " ]]; then
        echo -e "  ✅ SES Region is valid: $ses_region"
    else
        echo -e "  ⚠️  SES Region may be invalid or not supported: $ses_region"
        warnings+=("SES Region '$ses_region' may not support SES")
    fi
fi

# Check deployment status
echo ""
echo -e "${YELLOW}🚀 Checking deployment status...${NC}"
if kubectl get deployment "arda-credit-server" -n "$NAMESPACE" >/dev/null 2>&1; then
    ready_replicas=$(kubectl get deployment "arda-credit-server" -n "$NAMESPACE" -o jsonpath="{.status.readyReplicas}" 2>/dev/null || echo "0")
    desired_replicas=$(kubectl get deployment "arda-credit-server" -n "$NAMESPACE" -o jsonpath="{.spec.replicas}" 2>/dev/null || echo "0")
    
    if [[ "$ready_replicas" == "$desired_replicas" && "$ready_replicas" -gt 0 ]]; then
        echo -e "  ✅ Server deployment is ready ($ready_replicas/$desired_replicas replicas)"
    else
        echo -e "  ⚠️  Server deployment not fully ready ($ready_replicas/$desired_replicas replicas)"
        warnings+=("Server deployment not fully ready")
    fi
else
    echo -e "  ⚠️  Server deployment not found (may not be deployed yet)"
fi

# Display warnings
if [[ ${#warnings[@]} -gt 0 ]]; then
    echo ""
    echo -e "${YELLOW}⚠️  Warnings:${NC}"
    for warning in "${warnings[@]}"; do
        echo -e "  - $warning"
    done
fi

# Final validation result
echo ""
echo "===========================================" 
if [[ "$validation_passed" == "true" ]]; then
    echo -e "${GREEN}✅ AWS/SES Configuration Validation PASSED${NC}"
    echo ""
    echo -e "${BLUE}🎉 Your Arda Credit deployment is properly configured for Magic Link authentication!${NC}"
    echo ""
    echo -e "${YELLOW}Next Steps:${NC}"
    echo "1. Verify your email addresses in AWS SES Console"
    echo "2. Test Magic Link functionality"
    echo "3. Monitor application logs for any issues"
    echo ""
    echo -e "${BLUE}Test Magic Link:${NC}"
    echo "kubectl port-forward svc/arda-credit-server 8080:8080 -n $NAMESPACE &"
    echo "curl -X POST http://localhost:8080/api/v1/auth/magic-link \\"
    echo "  -H 'Content-Type: application/json' \\"
    echo "  -d '{\"email\":\"test@yourdomain.com\"}'"
    exit 0
else
    echo -e "${RED}❌ AWS/SES Configuration Validation FAILED${NC}"
    echo ""
    echo -e "${YELLOW}Please fix the above issues and run the validation again.${NC}"
    exit 1
fi