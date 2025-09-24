# Document Storage API Documentation

## Overview

The Arda Credit Document Storage system provides secure, role-based document management with zero-knowledge proof integration for privacy-preserving financial operations.

## Features

- **Company-Based Organization**: Documents organized by company accounts with team-based access
- **Role-Based Access Control**: Admin/Write/Read permissions with KYB verification gating
- **Secure Storage**: AWS S3 with AES-256 encryption and company-specific keys
- **Document Integrity**: SHA-256 hashing with integrity verification
- **ZK Proof Integration**: Document operations included in zero-knowledge state transitions
- **Audit Trail**: Complete audit logging for compliance and security
- **File Type Support**: PDF, DOC, DOCX, CSV with validation
- **Performance Monitoring**: Comprehensive metrics and alerting

## Authentication

All API endpoints require JWT authentication via the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

The JWT token must contain valid user claims with appropriate company membership and permissions.

## API Endpoints

### Upload Document

Upload a new document to company storage.

**Endpoint:** `POST /api/companies/{company_id}/documents/upload`

**Content-Type:** `multipart/form-data`

**Parameters:**
- `company_id` (path): UUID of the company
- `metadata` (form field): JSON metadata for the document
- `file` (form field): Binary file content

**Metadata Schema:**
```json
{
  "filename": "loan-contract.pdf",
  "document_type": "loan_contract",
  "deal_id": "optional-deal-uuid",
  "category": "contracts",
  "tags": ["loan", "contract", "q4-2024"],
  "access_level": "company",
  "metadata": {
    "custom_field": "value"
  }
}
```

**Response:**
```json
{
  "document": {
    "id": "doc-uuid",
    "company_id": "company-uuid",
    "filename": "loan-contract.pdf",
    "original_filename": "loan-contract.pdf",
    "file_extension": "pdf",
    "mime_type": "application/pdf",
    "file_size_bytes": 1048576,
    "document_type": "loan_contract",
    "deal_id": "deal-uuid",
    "category": "contracts",
    "tags": ["loan", "contract"],
    "sha256_hash": "abc123...",
    "access_level": "company",
    "approval_status": "pending",
    "status": "active",
    "version": 1,
    "uploaded_by": "user-uuid",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "s3_bucket": "arda-credit-documents",
    "s3_key": "company-uuid/deal-uuid/loan_contract/doc-uuid.pdf"
  },
  "message": "Document uploaded successfully"
}
```

**Required Permissions:** Upload documents (Write or Admin role)

### Download Document

Download a document with access control verification.

**Endpoint:** `GET /api/companies/{company_id}/documents/{document_id}/download`

**Parameters:**
- `company_id` (path): UUID of the company
- `document_id` (path): UUID of the document

**Response:** Binary file content with appropriate headers:
- `Content-Type`: Original MIME type
- `Content-Disposition`: `attachment; filename="original-filename.ext"`
- `Content-Length`: File size in bytes

**Required Permissions:** Download documents (Read, Write, or Admin role)

### Get Document Metadata

Retrieve document metadata without downloading content.

**Endpoint:** `GET /api/companies/{company_id}/documents/{document_id}`

**Response:** Document metadata object (same as upload response)

**Required Permissions:** View documents (Read, Write, or Admin role)

### List Documents

List documents for a company with filtering options.

**Endpoint:** `GET /api/companies/{company_id}/documents`

**Query Parameters:**
- `document_type` (optional): Filter by document type
- `deal_id` (optional): Filter by deal ID
- `status` (optional): Filter by status (active, archived, deleted)
- `approval_status` (optional): Filter by approval status (pending, approved, rejected)
- `limit` (optional): Maximum results (default: 50, max: 100)
- `offset` (optional): Pagination offset (default: 0)

**Response:**
```json
{
  "documents": [
    {
      "id": "doc-uuid",
      "filename": "document.pdf",
      // ... other document fields
    }
  ],
  "total_count": 150,
  "has_more": true
}
```

**Required Permissions:** View documents (Read, Write, or Admin role)

### Approve Document

Approve a pending document.

**Endpoint:** `POST /api/companies/{company_id}/documents/{document_id}/approve`

**Response:** Updated document metadata with approval information

**Required Permissions:** Approve documents (Write or Admin role)

### Reject Document

Reject a pending document.

**Endpoint:** `POST /api/companies/{company_id}/documents/{document_id}/reject`

**Response:** Updated document metadata with rejection information

**Required Permissions:** Approve documents (Write or Admin role)

### Delete Document

Soft delete a document (marks as deleted, doesn't remove from storage).

**Endpoint:** `DELETE /api/companies/{company_id}/documents/{document_id}`

**Response:** `204 No Content`

**Required Permissions:** Delete documents (Admin role only)

### Verify Document Integrity

Verify document integrity by comparing stored and calculated hashes.

**Endpoint:** `POST /api/companies/{company_id}/documents/{document_id}/verify`

**Response:**
```json
{
  "document_id": "doc-uuid",
  "is_valid": true,
  "message": "Document integrity verified successfully"
}
```

**Required Permissions:** View documents (Read, Write, or Admin role)

### Get Audit Logs

Retrieve document audit logs for compliance and security monitoring.

**Endpoint:** `GET /api/companies/{company_id}/documents/audit`

**Query Parameters:**
- `document_id` (optional): Filter by specific document
- `limit` (optional): Maximum results (default: 100)
- `offset` (optional): Pagination offset (default: 0)

**Response:**
```json
{
  "logs": [
    {
      "id": "log-uuid",
      "document_id": "doc-uuid",
      "company_id": "company-uuid",
      "user_id": "user-uuid",
      "action": "upload",
      "result": "success",
      "ip_address": "192.168.1.1",
      "user_agent": "Mozilla/5.0...",
      "request_id": "req-uuid",
      "details": {},
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "total_count": 50,
  "has_more": false
}
```

**Required Permissions:** View audit logs (Admin role only)

## Role-Based Access Control

### Team Roles

1. **Admin Role**
   - Upload documents
   - Download documents
   - Delete documents
   - Approve/reject documents
   - View audit logs
   - Manage team members

2. **Write Role**
   - Upload documents
   - Download documents
   - Approve/reject documents
   - Cannot delete documents
   - Cannot view audit logs

3. **Read Role**
   - Download documents only
   - Cannot upload, delete, or approve
   - Cannot view audit logs

### Access Levels

Documents have access levels that control visibility:

- **Public**: Accessible to all team members
- **Company**: Accessible to company team members only
- **Deal**: Accessible to deal participants only
- **Restricted**: Accessible to specific users only

### KYB/KYC Requirements

- **KYC Verification**: Required for all document operations
- **KYB Verification**: Required for sensitive operations (upload, delete, approve)

## File Type Support

Supported file types with validation:

- **PDF**: `application/pdf` (.pdf)
- **Microsoft Word**: `application/msword` (.doc)
- **Microsoft Word (OpenXML)**: `application/vnd.openxmlformats-officedocument.wordprocessingml.document` (.docx)
- **CSV**: `text/csv` (.csv)

### File Size Limits

- Maximum file size: 100MB per document
- Configurable per deployment environment

## Security Features

### Encryption

- **At Rest**: AES-256 encryption in AWS S3
- **In Transit**: HTTPS/TLS 1.3 for all API communications
- **Company-Specific Keys**: Optional KMS integration for company-specific encryption

### Document Integrity

- **SHA-256 Hashing**: All documents hashed on upload
- **Integrity Verification**: Hash verification on download
- **Document Fingerprinting**: Unique fingerprints for duplicate detection

### Audit Trail

Complete audit logging includes:
- User identification and authentication
- Document operations (upload, download, approve, delete)
- IP addresses and user agents
- Timestamps and request IDs
- Operation results and error details

### Rate Limiting

- Per-user rate limits to prevent abuse
- Company-level quotas for storage and operations
- Configurable limits based on subscription tier

## Zero-Knowledge Integration

### State Transitions

Document operations are integrated with ZK proof system:

- **Document Upload**: Creates state transition with document hash
- **Document Approval**: Updates state with approval information
- **Document Deletion**: Marks document as deleted in ZK state

### Privacy Preservation

- Document contents remain private
- Only document hashes and metadata included in proofs
- Batch processing for efficient proof generation

### Inclusion Proofs

Generate proofs that documents exist without revealing content:
- Merkle tree inclusion proofs
- State commitment verification
- Privacy-preserving audit capabilities

## Error Handling

### HTTP Status Codes

- `200 OK`: Successful operation
- `201 Created`: Document uploaded successfully
- `204 No Content`: Document deleted successfully
- `400 Bad Request`: Invalid request parameters or file
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Insufficient permissions or KYB/KYC not approved
- `404 Not Found`: Document, company, or user not found
- `409 Conflict`: Duplicate document or conflicting operation
- `413 Payload Too Large`: File size exceeds limits
- `415 Unsupported Media Type`: Invalid file type
- `429 Too Many Requests`: Rate limit exceeded
- `500 Internal Server Error`: Server error

### Error Response Format

```json
{
  "error": "error_code",
  "message": "Human-readable error message",
  "details": {
    "field": "Additional error details"
  }
}
```

### Common Error Codes

- `access_denied`: Insufficient permissions
- `company_not_found`: Company does not exist
- `document_not_found`: Document does not exist
- `user_not_found`: User does not exist
- `kyb_not_approved`: Company KYB verification required
- `kyc_not_approved`: User KYC verification required
- `invalid_file_type`: Unsupported file type
- `file_size_exceeded`: File too large
- `duplicate_document`: Document already exists
- `approval_required`: Document approval needed
- `validation_error`: Request validation failed

## Monitoring and Observability

### Metrics

Key metrics tracked:
- Document operation counts and success rates
- Upload/download performance and latency
- Storage usage and growth
- Security events and anomalies
- Error rates and types

### Alerts

Automated alerts for:
- High error rates (>5%)
- Slow performance (>5s average)
- Security events (unauthorized access, integrity failures)
- Storage quota approaching limits
- Unusual activity patterns

### Health Checks

Health check endpoint: `GET /api/documents/health`

Response:
```json
{
  "status": "healthy",
  "service": "document_service",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

## Deployment Configuration

### Environment Variables

Required environment variables:

```bash
# Database
DATABASE_URL=postgresql://user:pass@host:5432/arda_credit

# AWS Configuration
AWS_REGION=us-east-1
AWS_S3_BUCKET=arda-credit-documents
AWS_KMS_KEY_ID=optional-kms-key-id

# Application
DOCUMENT_MAX_FILE_SIZE_MB=100
DOCUMENT_STORAGE_LIFECYCLE_DAYS_IA=30
DOCUMENT_STORAGE_LIFECYCLE_DAYS_GLACIER=90
DOCUMENT_STORAGE_LIFECYCLE_DAYS_DELETE=2555

# Security
JWT_SECRET=your-jwt-secret
RATE_LIMIT_REQUESTS_PER_MINUTE=60

# Monitoring
METRICS_ENABLED=true
PROMETHEUS_ENDPOINT=/metrics
```

### AWS IAM Permissions

Required IAM permissions for S3 operations:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:ListBucket",
        "s3:GetObjectVersion",
        "s3:PutBucketLifecycleConfiguration"
      ],
      "Resource": [
        "arn:aws:s3:::arda-credit-documents",
        "arn:aws:s3:::arda-credit-documents/*"
      ]
    },
    {
      "Effect": "Allow",
      "Action": [
        "kms:Encrypt",
        "kms:Decrypt",
        "kms:ReEncrypt*",
        "kms:GenerateDataKey*",
        "kms:DescribeKey"
      ],
      "Resource": "arn:aws:kms:region:account:key/key-id"
    }
  ]
}
```

### Database Setup

Run database migrations:

```bash
sqlx migrate run --database-url $DATABASE_URL
```

### Docker Configuration

Example Dockerfile configuration:

```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/arda-credit /usr/local/bin/

EXPOSE 8080

CMD ["arda-credit"]
```

### Kubernetes Deployment

Example Kubernetes deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: arda-credit-document-service
spec:
  replicas: 3
  selector:
    matchLabels:
      app: arda-credit-document-service
  template:
    metadata:
      labels:
        app: arda-credit-document-service
    spec:
      containers:
      - name: document-service
        image: arda-credit:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: arda-credit-secrets
              key: database-url
        - name: AWS_REGION
          value: "us-east-1"
        - name: AWS_S3_BUCKET
          value: "arda-credit-documents"
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /api/documents/health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /api/documents/health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

## Testing

### Unit Tests

Run unit tests:

```bash
cargo test --lib
```

### Integration Tests

Run integration tests (requires test database):

```bash
export TEST_DATABASE_URL=postgresql://test:test@localhost/arda_credit_test
cargo test --test document_integration_tests
```

### Performance Tests

Run performance tests:

```bash
cargo test --test document_integration_tests --release -- --ignored test_document_performance
```

## Security Considerations

### Data Protection

- All sensitive data encrypted at rest and in transit
- Company-specific encryption keys for enhanced security
- Regular security audits and penetration testing

### Access Control

- Multi-factor authentication for admin operations
- Regular access reviews and permission audits
- Principle of least privilege enforcement

### Compliance

- GDPR compliance for data retention and deletion
- SOC 2 Type II compliance for security controls
- Regular compliance audits and certifications

### Incident Response

- 24/7 security monitoring and alerting
- Incident response procedures and escalation
- Regular security training and awareness programs

## Support and Maintenance

### Monitoring

- Real-time monitoring of all document operations
- Performance metrics and alerting
- Security event monitoring and response

### Backup and Recovery

- Automated daily backups of document metadata
- Cross-region replication for disaster recovery
- Regular backup testing and recovery procedures

### Updates and Maintenance

- Regular security updates and patches
- Scheduled maintenance windows with advance notice
- Version control and rollback procedures

For technical support or questions, contact the Arda Credit development team.