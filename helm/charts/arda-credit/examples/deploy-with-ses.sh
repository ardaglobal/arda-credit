#!/bin/bash

# Deploy Arda Credit with AWS/SES Magic Link Support
# This script demonstrates how to deploy the Arda Credit HELM chart with complete AWS/SES configuration

set -euo pipefail

# Configuration
NAMESPACE="arda-credit"
RELEASE_NAME="arda-credit"
CHART_PATH="./helm/charts/arda-credit"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Deploying Arda Credit with AWS/SES Magic Link Support${NC}"
echo ""

# Check required environment variables
required_vars=(
    "SINDRI_API_KEY"
    "ETHEREUM_RPC_URL"
    "ETHEREUM_CONTRACT_ADDRESS"
    "ETHEREUM_WALLET_PRIVATE_KEY"
    "ETHEREUM_DEPLOYER_ADDRESS"
    "AWS_ACCESS_KEY_ID"
    "AWS_SECRET_ACCESS_KEY"
    "EMAIL_FROM"
    "EMAIL_REPLY_TO"
    "GITHUB_ORG_PAT"
)

echo -e "${YELLOW}📋 Checking required environment variables...${NC}"
missing_vars=()

for var in "${required_vars[@]}"; do
    if [[ -z "${!var:-}" ]]; then
        missing_vars+=("$var")
    else
        echo -e "  ✅ $var is set"
    fi
done

if [[ ${#missing_vars[@]} -gt 0 ]]; then
    echo -e "${RED}❌ Missing required environment variables:${NC}"
    for var in "${missing_vars[@]}"; do
        echo -e "  - $var"
    done
    echo ""
    echo -e "${YELLOW}Please set the missing variables and run again.${NC}"
    echo ""
    echo "Example:"
    echo "export AWS_ACCESS_KEY_ID='your-aws-access-key'"
    echo "export AWS_SECRET_ACCESS_KEY='your-aws-secret-key'"
    echo "export EMAIL_FROM='noreply@arda.xyz'"
    echo "export EMAIL_REPLY_TO='support@arda.xyz'"
    exit 1
fi

echo -e "${GREEN}✅ All required environment variables are set${NC}"
echo ""

# Create namespace if it doesn't exist
echo -e "${BLUE}📦 Creating namespace: $NAMESPACE${NC}"
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -

# Create GHCR secret for pulling private images
echo -e "${BLUE}🔐 Creating GHCR authentication secret...${NC}"
kubectl create secret docker-registry ghcr-secret \
    --docker-server=ghcr.io \
    --docker-username=arda-bot \
    --docker-password="$GITHUB_ORG_PAT" \
    --namespace="$NAMESPACE" \
    --dry-run=client -o yaml | kubectl apply -f -

# Create application secrets with AWS/SES configuration
echo -e "${BLUE}🔑 Creating application secrets with AWS/SES configuration...${NC}"
kubectl create secret generic arda-credit-secrets \
    --from-literal=sindri-api-key="$SINDRI_API_KEY" \
    --from-literal=ethereum-rpc-url="$ETHEREUM_RPC_URL" \
    --from-literal=ethereum-contract-address="$ETHEREUM_CONTRACT_ADDRESS" \
    --from-literal=ethereum-wallet-private-key="$ETHEREUM_WALLET_PRIVATE_KEY" \
    --from-literal=ethereum-deployer-address="$ETHEREUM_DEPLOYER_ADDRESS" \
    --from-literal=aws-access-key-id="$AWS_ACCESS_KEY_ID" \
    --from-literal=aws-secret-access-key="$AWS_SECRET_ACCESS_KEY" \
    --from-literal=email-from="$EMAIL_FROM" \
    --from-literal=email-reply-to="$EMAIL_REPLY_TO" \
    --namespace="$NAMESPACE" \
    --dry-run=client -o yaml | kubectl apply -f -

# Add Bitnami repository for PostgreSQL
echo -e "${BLUE}📚 Adding Bitnami Helm repository...${NC}"
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Update chart dependencies
echo -e "${BLUE}📦 Updating Helm chart dependencies...${NC}"
helm dependency update "$CHART_PATH"

# Deploy the chart with AWS/SES configuration
echo -e "${BLUE}🚀 Deploying Arda Credit with AWS/SES configuration...${NC}"

# Optional: Set custom values
SES_REGION="${SES_REGION:-us-east-1}"
MAGIC_BASE_URL="${MAGIC_BASE_URL:-https://app.arda.xyz/auth/verify}"
EMAIL_ENABLED="${EMAIL_ENABLED:-true}"
EMAIL_DEBUG_MODE="${EMAIL_DEBUG_MODE:-false}"
INGRESS_HOST="${INGRESS_HOST:-app.stag.aws.arda.xyz}"

helm upgrade --install "$RELEASE_NAME" "$CHART_PATH" \
    --namespace="$NAMESPACE" \
    --set postgresql.auth.password="$(openssl rand -base64 32)" \
    --set server.config.sesRegion="$SES_REGION" \
    --set server.config.magicBaseUrl="$MAGIC_BASE_URL" \
    --set server.config.emailEnabled="$EMAIL_ENABLED" \
    --set server.config.emailDebugMode="$EMAIL_DEBUG_MODE" \
    --set ingress.hosts[0].host="$INGRESS_HOST" \
    --set secrets.existingSecret="" \
    --timeout=10m \
    --wait

echo ""
echo -e "${GREEN}✅ Deployment completed successfully!${NC}"
echo ""

# Show deployment status
echo -e "${BLUE}📊 Deployment Status:${NC}"
kubectl get pods,svc,ingress -n "$NAMESPACE"

echo ""
echo -e "${BLUE}🔗 Access Information:${NC}"
echo -e "  Frontend: https://$INGRESS_HOST"
echo -e "  API: https://$INGRESS_HOST/api/v1"
echo -e "  Health Check: https://$INGRESS_HOST/api/v1/health"
echo -e "  Magic Link Test: https://$INGRESS_HOST/api/v1/auth/magic-link"

echo ""
echo -e "${YELLOW}📧 AWS/SES Configuration:${NC}"
echo -e "  Region: $SES_REGION"
echo -e "  Email From: $EMAIL_FROM"
echo -e "  Email Reply-To: $EMAIL_REPLY_TO"
echo -e "  Magic Link Base URL: $MAGIC_BASE_URL"
echo -e "  Email Enabled: $EMAIL_ENABLED"
echo -e "  Debug Mode: $EMAIL_DEBUG_MODE"

echo ""
echo -e "${GREEN}🎉 Arda Credit is now running with Magic Link authentication!${NC}"
echo ""
echo -e "${YELLOW}💡 Next Steps:${NC}"
echo "1. Verify your SES email addresses in AWS Console"
echo "2. Test Magic Link functionality:"
echo "   curl -X POST https://$INGRESS_HOST/api/v1/auth/magic-link \\"
echo "     -H 'Content-Type: application/json' \\"
echo "     -d '{\"email\":\"test@yourdomain.com\"}'"
echo "3. Monitor logs: kubectl logs -f deployment/arda-credit-server -n $NAMESPACE"