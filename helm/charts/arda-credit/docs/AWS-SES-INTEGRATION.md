# AWS SES Integration for Arda Credit Magic Link Authentication

This document provides comprehensive guidance for integrating AWS Simple Email Service (SES) with the Arda Credit HELM chart to enable Magic Link authentication.

## Overview

The Arda Credit application uses Magic Link authentication for passwordless user access. This requires AWS SES for sending authentication emails. This integration includes:

- **Secure credential management** via Kubernetes secrets
- **Flexible configuration** through ConfigMaps and values
- **Production-ready deployment** patterns
- **Comprehensive validation** and monitoring

## Prerequisites

### AWS SES Setup

1. **AWS Account** with SES access
2. **Verified email addresses** or domains in SES
3. **IAM user** with appropriate SES permissions
4. **SES region** configured (default: us-east-1)

### Kubernetes Requirements

- Kubernetes 1.20+
- HELM 3.8+
- Appropriate RBAC permissions
- Access to create secrets and configmaps

## AWS SES Configuration

### 1. Verify Email Addresses/Domains

In the AWS SES Console:

1. Navigate to **Verified identities**
2. Add and verify your sender email addresses:
   - `noreply@arda.xyz` (or your domain)
   - `support@arda.xyz` (for replies)

### 2. Create IAM User and Policy

Create an IAM user with the following policy:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "ArdaCreditSESPermissions",
      "Effect": "Allow",
      "Action": [
        "ses:SendEmail",
        "ses:SendRawEmail",
        "ses:GetAccount",
        "ses:GetSendQuota",
        "ses:GetSendStatistics"
      ],
      "Resource": "*"
    }
  ]
}
```

### 3. Generate Access Keys

1. Create access keys for the IAM user
2. Store them securely (they'll be added to Kubernetes secrets)

### 4. SES Configuration Set (Optional)

For advanced tracking and monitoring:

1. Create a configuration set in SES
2. Configure event publishing (SNS/CloudWatch)
3. Add the configuration set name to your HELM values

## HELM Chart Configuration

### Environment Variables

The chart uses the following environment variables for SES configuration:

#### Sensitive (stored in secrets):
- `AWS_ACCESS_KEY_ID`: AWS access key
- `AWS_SECRET_ACCESS_KEY`: AWS secret access key
- `EMAIL_FROM`: Verified sender email address
- `EMAIL_REPLY_TO`: Reply-to email address

#### Non-sensitive (stored in ConfigMap):
- `SES_REGION`: AWS region (default: "us-east-1")
- `SES_CONFIGURATION_SET`: Optional configuration set name
- `MAGIC_BASE_URL`: Base URL for magic links
- `EMAIL_ENABLED`: Enable/disable email service
- `EMAIL_DEBUG_MODE`: Debug mode (logs links instead of sending)

### Values.yaml Configuration

```yaml
# Server configuration
server:
  config:
    # AWS/SES Configuration (non-sensitive)
    sesRegion: "us-east-1"
    sesConfigurationSet: "arda-credit-production"  # Optional
    magicBaseUrl: "https://app.arda.xyz/auth/verify"
    emailEnabled: "true"
    emailDebugMode: "false"  # Set to "true" for testing

# Application secrets
secrets:
  app:
    # AWS/SES Configuration (sensitive)
    awsAccessKeyId: "your-aws-access-key-id"
    awsSecretAccessKey: "your-aws-secret-access-key"
    emailFrom: "noreply@arda.xyz"
    emailReplyTo: "support@arda.xyz"
```

## Deployment Guide

### Option 1: Using the Deployment Script

Use the provided deployment script for automated setup:

```bash
# Set required environment variables
export AWS_ACCESS_KEY_ID="your-aws-access-key"
export AWS_SECRET_ACCESS_KEY="your-aws-secret-key"
export EMAIL_FROM="noreply@arda.xyz"
export EMAIL_REPLY_TO="support@arda.xyz"
export GITHUB_ORG_PAT="your-github-pat"

# Set other required variables...
export SINDRI_API_KEY="your-sindri-key"
export ETHEREUM_RPC_URL="your-ethereum-rpc"
# ... etc

# Run deployment script
./helm/charts/arda-credit/examples/deploy-with-ses.sh
```

### Option 2: Manual Deployment

1. **Create secrets:**

```bash
kubectl create secret generic arda-credit-secrets \
  --from-literal=aws-access-key-id="$AWS_ACCESS_KEY_ID" \
  --from-literal=aws-secret-access-key="$AWS_SECRET_ACCESS_KEY" \
  --from-literal=email-from="$EMAIL_FROM" \
  --from-literal=email-reply-to="$EMAIL_REPLY_TO" \
  --from-literal=sindri-api-key="$SINDRI_API_KEY" \
  --from-literal=ethereum-rpc-url="$ETHEREUM_RPC_URL" \
  --from-literal=ethereum-contract-address="$ETHEREUM_CONTRACT_ADDRESS" \
  --from-literal=ethereum-wallet-private-key="$ETHEREUM_WALLET_PRIVATE_KEY" \
  --from-literal=ethereum-deployer-address="$ETHEREUM_DEPLOYER_ADDRESS" \
  --namespace=arda-credit
```

2. **Deploy with HELM:**

```bash
helm upgrade --install arda-credit ./helm/charts/arda-credit \
  --namespace=arda-credit \
  --set server.config.sesRegion="us-east-1" \
  --set server.config.emailEnabled="true" \
  --set server.config.magicBaseUrl="https://app.arda.xyz/auth/verify" \
  --create-namespace \
  --wait
```

### Option 3: Using Production Values File

```bash
helm upgrade --install arda-credit ./helm/charts/arda-credit \
  --namespace=arda-credit \
  --values=./helm/charts/arda-credit/examples/values-ses-production.yaml \
  --create-namespace \
  --wait
```

## Validation and Testing

### 1. Configuration Validation

Run the validation script to ensure everything is configured correctly:

```bash
./helm/charts/arda-credit/scripts/validate-ses-config.sh
```

### 2. Manual Testing

Test the Magic Link functionality:

```bash
# Port forward to access the API
kubectl port-forward svc/arda-credit-server 8080:8080 -n arda-credit &

# Test magic link request
curl -X POST http://localhost:8080/api/v1/auth/magic-link \
  -H "Content-Type: application/json" \
  -d '{"email":"test@yourdomain.com"}'
```

Expected response:
```json
{
  "success": true,
  "message": "Magic link sent to your email address",
  "message_id": "ses-message-id-123",
  "expires_in_minutes": 10,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 3. Debug Mode Testing

For development/testing, enable debug mode:

```bash
helm upgrade arda-credit ./helm/charts/arda-credit \
  --set server.config.emailDebugMode="true" \
  --reuse-values
```

In debug mode, magic links are logged instead of sent via email.

## Monitoring and Troubleshooting

### Application Logs

Monitor the application logs for SES-related messages:

```bash
kubectl logs -f deployment/arda-credit-server -n arda-credit | grep -E "(SES|email|magic)"
```

### Common Log Messages

- `✅ AWS SES client initialized successfully` - SES client ready
- `📧 Sending magic link email to: user@example.com` - Email being sent
- `✅ Magic link email sent successfully (message_id: xxx)` - Email sent
- `❌ SES send failed with detailed error: xxx` - SES error

### Common Issues and Solutions

#### 1. MessageRejected Error
```
❌ MessageRejected: Email was rejected by SES
```
**Solution:** Verify sender email address in SES console

#### 2. SendingPausedException
```
⏸️ SendingPausedException: SES sending is paused
```
**Solution:** Check SES account status and sending limits

#### 3. AccessDenied Error
```
🛡️ AccessDenied: Insufficient SES permissions
```
**Solution:** Verify IAM policy includes required SES permissions

#### 4. SignatureDoesNotMatch
```
🔑 SignatureDoesNotMatch: AWS credentials issue
```
**Solution:** Verify AWS access key and secret access key

### SES Metrics and Monitoring

Monitor SES usage through:

1. **AWS CloudWatch:** SES metrics (sends, bounces, complaints)
2. **Application metrics:** Custom Prometheus metrics
3. **SES Console:** Sending statistics and reputation

### Health Checks

The application includes SES connectivity health checks:

```bash
curl http://localhost:8080/api/v1/health
```

## Security Considerations

### Credential Management

1. **Never commit credentials** to version control
2. **Use Kubernetes secrets** for sensitive data
3. **Rotate credentials regularly**
4. **Use least-privilege IAM policies**

### Email Security

1. **Verify all sender domains** in SES
2. **Use DKIM signing** for authenticity
3. **Monitor bounce and complaint rates**
4. **Implement SPF/DMARC records**

### Network Security

1. **Use TLS/HTTPS** for all communications
2. **Restrict SES access** to application pods only
3. **Monitor unusual sending patterns**

## Production Considerations

### Scaling

- SES has sending limits (start with sandbox limits)
- Request limit increases for production workloads
- Monitor sending quotas and rates

### High Availability

- Deploy across multiple availability zones
- Use SES in multiple regions for redundancy
- Implement retry logic for transient failures

### Cost Optimization

- Monitor SES usage and costs
- Use SES configuration sets for detailed tracking
- Optimize email content size

### Compliance

- Implement proper unsubscribe mechanisms
- Maintain sending reputation
- Follow CAN-SPAM and GDPR requirements

## Migration from Other Email Providers

If migrating from another email service:

1. **Update environment variables** to use SES configuration
2. **Verify email templates** work with SES
3. **Test thoroughly** in staging environment
4. **Monitor bounce rates** after migration
5. **Update DNS records** if changing domains

## Support and Resources

### AWS SES Documentation
- [AWS SES Developer Guide](https://docs.aws.amazon.com/ses/)
- [SES API Reference](https://docs.aws.amazon.com/ses/latest/APIReference/)
- [SES Best Practices](https://docs.aws.amazon.com/ses/latest/dg/best-practices.html)

### Troubleshooting Resources
- [SES Troubleshooting Guide](https://docs.aws.amazon.com/ses/latest/dg/troubleshooting.html)
- [SES Error Codes](https://docs.aws.amazon.com/ses/latest/APIReference/CommonErrors.html)

### Arda Credit Specific
- Check application logs for detailed error messages
- Use the validation script for configuration issues
- Contact support with SES message IDs for tracking

---

This integration enables secure, scalable Magic Link authentication for Arda Credit using AWS SES, following industry best practices for security, monitoring, and deployment.