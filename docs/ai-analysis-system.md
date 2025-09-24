# AI Document Analysis and Negotiation Assistant

A comprehensive AI-powered system for analyzing document changes, providing negotiation insights, and facilitating intelligent document review through conversational interfaces.

## Overview

The AI Analysis System provides:

- **Document Change Analysis**: Automated detection and classification of changes between document versions
- **Risk Assessment**: AI-powered evaluation of change impact and business risk
- **Conversational AI**: Chat interface for document insights and negotiation advice
- **Smart Notifications**: Real-time alerts for critical changes and insights
- **Analytics Dashboard**: Comprehensive view of document analysis trends and metrics
- **Multi-LLM Support**: Integration with OpenAI, Anthropic, Azure, and local models

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Frontend UI   │    │  API Handlers   │    │  AI Services    │
│                 │    │                 │    │                 │
│ • Chat Interface│◄──►│ • Analysis API  │◄──►│ • LLM Service   │
│ • Document View │    │ • Chat API      │    │ • Analysis Svc  │
│ • Analytics     │    │ • Notifications │    │ • Notification  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                ▲                        ▲
                                │                        │
                                ▼                        ▼
                       ┌─────────────────┐    ┌─────────────────┐
                       │   Database      │    │  External APIs  │
                       │                 │    │                 │
                       │ • Analysis Data │    │ • OpenAI        │
                       │ • Chat History  │    │ • Anthropic     │
                       │ • Notifications │    │ • Azure OpenAI  │
                       └─────────────────┘    └─────────────────┘
```

## Key Features

### 1. Document Diff Analysis

**Endpoint**: `GET /api/v1/documents/{id}/analyze`

Analyzes differences between document versions and provides structured insights:

```rust
pub struct DocumentDiffResponse {
    pub document_id: Uuid,
    pub analysis_id: Uuid,
    pub changes: Vec<ChangeAnalysis>,
    pub overall_risk_score: f64,
    pub summary: String,
    pub critical_changes_count: i32,
    pub processed_at: DateTime<Utc>,
}
```

**Change Classification**:
- **Type**: Addition, Deletion, Modification, Restructure
- **Impact**: Minor, Moderate, Significant, Critical
- **Category**: Financial, Legal, Operational, Compliance
- **Risk Factors**: Identified risks with mitigation suggestions

### 2. Conversational AI Interface

**Endpoint**: `POST /api/v1/ai/chat`

Provides natural language interface for document analysis questions:

```rust
pub struct AiChatRequest {
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub message: String,
    pub context: Option<serde_json::Value>,
}
```

**Example Interactions**:

**User**: "What changed in the interest rate section?"

**AI**: "The interest rate increased from 8.5% to 9.25% annually (0.75% increase). This represents approximately $15,000 additional annual interest cost on the $2M principal. The change also includes a new step-up provision at year 3, which is not market standard for this loan type. I recommend negotiating either a lower base rate or removing the step-up clause."

### 3. Negotiation Insights

**Endpoint**: `GET /api/v1/documents/{id}/insights`

Generates strategic insights for document negotiation:

```rust
pub struct NegotiationInsight {
    pub insight_type: String,      // market_comparison, risk_assessment, recommendation
    pub title: String,
    pub description: String,
    pub market_comparison: Option<String>,
    pub recommended_action: Option<String>,
    pub priority: ImpactLevel,
}
```

### 4. Smart Notifications

**Real-time Alerts**:
- Critical change detection
- High-risk modifications
- Financial/legal term changes
- Analysis completion notifications

**Delivery Methods**:
- In-app notifications
- Email alerts
- Webhook endpoints
- SMS (configurable)

### 5. Analytics Dashboard

**Endpoint**: `GET /api/v1/ai/analytics/dashboard`

Provides comprehensive analytics:

```json
{
  "total_analyses": 156,
  "critical_changes_today": 8,
  "average_risk_score": 0.67,
  "most_active_documents": [...],
  "change_categories": {
    "financial": 45,
    "legal": 32,
    "operational": 18,
    "compliance": 12
  },
  "risk_trends": [...]
}
```

## API Endpoints

### Document Analysis
- `GET /api/v1/documents/{id}/analyze` - Analyze document changes
- `POST /api/v1/documents/{id}/trigger-analysis` - Trigger real-time analysis
- `GET /api/v1/documents/{id}/analysis-history` - Get analysis history

### AI Chat
- `POST /api/v1/ai/chat` - Send chat message
- `GET /api/v1/ai/chat/{session_id}/history` - Get chat history

### Insights & Analytics
- `GET /api/v1/documents/{id}/insights` - Get negotiation insights
- `GET /api/v1/ai/analytics/dashboard` - Analytics dashboard

### Notifications
- `GET /api/v1/notifications` - Get user notifications
- `PUT /api/v1/notifications/{id}/read` - Mark notification as read
- `POST /api/v1/notification-preferences` - Update preferences

## Configuration

### Environment Variables

```bash
# AI Provider Configuration
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=...
AZURE_OPENAI_KEY=...
AZURE_OPENAI_ENDPOINT=...

# Analysis Settings
AI_PROVIDER=openai  # openai, anthropic, azure, local
AI_MODEL=gpt-4     # Model to use
AI_TEMPERATURE=0.3  # Lower for more consistent analysis
AI_MAX_TOKENS=4000

# Notification Settings
NOTIFICATION_EMAIL_ENABLED=true
NOTIFICATION_WEBHOOK_ENABLED=true
NOTIFICATION_RATE_LIMIT_PER_DAY=50
```

### LLM Provider Setup

**OpenAI**:
```rust
let ai_service = LlmService::new(LlmProvider::OpenAi)?;
```

**Anthropic Claude**:
```rust
let ai_service = LlmService::new(LlmProvider::Anthropic)?;
```

**Azure OpenAI**:
```rust
let ai_service = LlmService::new(LlmProvider::Azure)?;
```

**Local Model**:
```rust
let ai_service = LlmService::new(LlmProvider::Local("http://localhost:8080".to_string()))?;
```

## Database Schema

### Core Analysis Tables

```sql
-- AI document analyses
CREATE TABLE ai_document_analyses (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    overall_risk_score DECIMAL(3,2),
    total_changes INTEGER,
    created_at TIMESTAMP WITH TIME ZONE
);

-- Individual change analyses
CREATE TABLE ai_change_analyses (
    id UUID PRIMARY KEY,
    analysis_id UUID NOT NULL,
    change_type VARCHAR(20),  -- addition, deletion, modification, restructure
    impact_level VARCHAR(20), -- minor, moderate, significant, critical
    category VARCHAR(20),     -- financial, legal, operational, compliance
    summary TEXT,
    confidence_score DECIMAL(3,2)
);

-- AI chat conversations
CREATE TABLE ai_chat_messages (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    user_id UUID NOT NULL,
    role VARCHAR(20),     -- user, assistant
    content TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE
);
```

### Notification System Tables

```sql
-- Notification preferences
CREATE TABLE user_notification_preferences (
    user_id UUID PRIMARY KEY,
    enabled_notifications JSONB,
    delivery_preferences JSONB,
    quiet_hours_start TIME,
    quiet_hours_end TIME
);

-- Webhook configurations
CREATE TABLE user_webhook_configs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    url TEXT NOT NULL,
    secret VARCHAR(255),
    notification_types JSONB
);
```

## Usage Examples

### Analyze Document Changes

```typescript
// Trigger analysis
const response = await fetch('/api/v1/documents/doc-123/analyze', {
  method: 'GET',
  headers: { 'Authorization': `Bearer ${token}` }
});

const analysis = await response.json();
console.log(`Found ${analysis.changes.length} changes`);
console.log(`Risk score: ${analysis.overall_risk_score}`);
```

### Chat with AI about Document

```typescript
// Send chat message
const chatResponse = await fetch('/api/v1/ai/chat', {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${token}`,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    document_id: 'doc-123',
    message: 'Explain the financial impact of recent changes'
  })
});

const aiResponse = await chatResponse.json();
console.log(aiResponse.response);
```

### Configure Webhooks

```typescript
// Set up webhook for critical alerts
const webhookConfig = {
  name: "Critical Changes Webhook",
  url: "https://myapp.com/ai-notifications",
  secret: "webhook-secret-123",
  notification_types: ["critical_changes_alert", "high_risk_alert"]
};

await fetch('/api/v1/webhook-configs', {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${token}`,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify(webhookConfig)
});
```

## AI Prompt Engineering

### Document Analysis Prompts

The system uses carefully crafted prompts for consistent analysis:

```text
Analyze the following document changes and provide a structured analysis:

CURRENT DOCUMENT:
[document content]

PREVIOUS VERSION:
[previous content]

DOCUMENT TYPE: loan_agreement
FOCUS AREAS: financial, legal

Provide analysis in JSON format with the following structure for each change identified:
{
  "changes": [
    {
      "change_type": "addition|deletion|modification|restructure",
      "impact_level": "minor|moderate|significant|critical",
      "category": "financial|legal|operational|compliance",
      "summary": "Brief description of the change",
      "implications": ["List of implications"],
      "recommendations": ["List of recommendations"],
      "related_clauses": ["Related document sections"],
      "risk_factors": [...],
      "confidence_score": 0.85
    }
  ]
}
```

### Chat System Prompts

```text
You are an AI assistant specializing in document analysis and negotiation support.

Your capabilities include:
- Explaining document changes in plain language
- Providing risk assessments and recommendations
- Comparing terms to market standards
- Offering negotiation strategies
- Answering questions about legal and financial implications

Be helpful, accurate, and concise. If you're uncertain about something, acknowledge it.
Focus on practical advice that helps users make informed decisions.
```

## Monitoring & Performance

### Key Metrics

- **Analysis Throughput**: Documents processed per hour
- **AI Response Time**: Average time for analysis completion
- **Accuracy Scores**: User feedback on AI analysis quality
- **Notification Delivery**: Success rates by delivery method
- **Cost Tracking**: Token usage and API costs by provider

### Performance Optimization

- **Caching**: Analysis results cached for repeated requests
- **Batch Processing**: Multiple documents analyzed together
- **Model Selection**: Different models for different analysis types
- **Rate Limiting**: API usage controls to manage costs

## Security & Privacy

### Data Protection

- **No Data Leakage**: Each analysis is isolated per user/organization
- **Encryption**: All document content encrypted in transit and at rest
- **Access Controls**: Role-based permissions for analysis results
- **Audit Logging**: Complete trail of all AI interactions

### Privacy Controls

- **Data Retention**: Configurable retention periods for chat history
- **Anonymization**: Option to anonymize content before AI processing
- **Local Models**: Support for on-premises AI models
- **GDPR Compliance**: Data deletion and export capabilities

## Troubleshooting

### Common Issues

**Analysis Not Triggering**:
- Check document permissions
- Verify webhook configurations
- Review notification preferences

**AI Responses Seem Inaccurate**:
- Adjust model temperature settings
- Provide more document context
- Submit feedback for model improvement

**Notifications Not Delivered**:
- Check user notification preferences
- Verify webhook endpoint accessibility
- Review quiet hours settings

### Debugging Tools

- **Analysis Logs**: Detailed logs of AI processing steps
- **Webhook Testing**: Test webhook delivery endpoints
- **Chat Debug**: View conversation context and prompts
- **Performance Metrics**: Analysis timing and token usage

## Development & Testing

### Local Development

```bash
# Start with local AI model
export AI_PROVIDER=local
export LOCAL_AI_URL=http://localhost:8080
cargo run --bin server

# Or use OpenAI for development
export AI_PROVIDER=openai
export OPENAI_API_KEY=sk-...
cargo run --bin server
```

### Testing

```bash
# Run AI analysis tests
cargo test ai_analysis

# Test notification delivery
cargo test notification_service

# Integration tests
cargo test ai_integration
```

### Mock AI Responses

For testing without real AI APIs:

```rust
// Mock analysis for testing
let mock_analysis = DocumentDiffResponse {
    changes: vec![
        ChangeAnalysis {
            change_type: ChangeType::Modification,
            impact_level: ImpactLevel::Significant,
            category: ChangeCategory::Financial,
            summary: "Interest rate increased".to_string(),
            // ...
        }
    ],
    overall_risk_score: 0.75,
    // ...
};
```

## Roadmap

### Phase 1: Core Analysis (✅ Complete)
- Document diff analysis
- Change categorization
- Basic AI chat interface
- Notification system

### Phase 2: Advanced Features (In Progress)
- Multi-document analysis
- Historical trend analysis
- Advanced negotiation strategies
- Custom AI model fine-tuning

### Phase 3: Enterprise Features (Planned)
- Multi-tenant isolation
- Advanced analytics
- API rate limiting
- Enterprise SSO integration

### Phase 4: AI Enhancement (Planned)
- Specialized legal/financial models
- Industry-specific analysis
- Automated contract generation
- Predictive risk modeling

## Support

For questions or issues with the AI Analysis System:

- **Documentation**: `/docs/ai-analysis-system.md`
- **API Reference**: `/api/v1/docs` (Swagger UI)
- **GitHub Issues**: Report bugs and feature requests
- **Chat Support**: Use the AI assistant for self-service help

---

The AI Document Analysis System provides powerful automation for document review and negotiation, enabling faster decision-making and better risk management throughout the deal lifecycle.