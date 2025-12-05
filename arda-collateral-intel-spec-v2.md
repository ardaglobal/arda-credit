# Arda Collateral Intelligence Service
## Technical Specification v2.0

**Version:** 2.0  
**Date:** December 4, 2025  
**Author:** Claude (with Matt)  
**Status:** Draft for Review

---

## Executive Summary

`arda-collateral-intel` is a dedicated Rust microservice that provides AI-powered collateral double-pledge detection. It maintains its own **knowledge graph** of collateral relationships, deal lifecycle status, and detection history—enabling increasingly intelligent and efficient detection over time.

### Core Design Principles

| Principle | Implementation |
|-----------|----------------|
| **Service Isolation** | Communicate via APIs only; no direct DB access to other services |
| **Own Knowledge Store** | SurrealDB instance for collateral knowledge graph |
| **API-Driven** | Consume arda-credit API and arda-ingest API (or MCP) |
| **Event-Sourced** | Build understanding from events; arda-credit is dumb publisher |
| **Deal Lifecycle Aware** | Understand "in-use" vs "not-in-use" collateral states |

---

## 1. Document Vector Analysis: Option Comparison

### Option A: arda-ingest API

**Current State**: arda-ingest is a Rust service with REST API endpoints for semantic search.

| Aspect | Details |
|--------|---------|
| **API Endpoints** | `/api/semantic-search`, `/api/documents/{id}` |
| **Database** | SurrealDB (shared instance with document storage) |
| **Embeddings** | Qwen3-Embedding-8B via Modal (4096 dimensions) |
| **Deployment** | Single service, environment-specific configs |
| **Pros** | Direct Rust-to-Rust HTTP calls; already deployed; lower latency |
| **Cons** | Tightly coupled to document ingestion; API may need extension |

### Option B: mcp-arda-vector-documents (FastMCP)

**Current State**: Python FastMCP server providing MCP tools for document vector search.

| Aspect | Details |
|--------|---------|
| **Tools Available** | `semantic_search_credit_documents`, `search_credit_documents_by_metadata`, `get_document_embeddings_stats` |
| **Database** | SurrealDB (same as arda-ingest) + PostgreSQL (for taxonomy) |
| **Embeddings** | Same Qwen3-Embedding-8B via Modal |
| **Deployment** | FastMCP Cloud per environment (local, dev, staging, prod) |
| **Pros** | Environment-specific deployments; MCP protocol standardization; additional tools (covenant analysis, taxonomy) |
| **Cons** | Additional network hop (MCP protocol); Python service adds diversity |

### Recommendation

**Hybrid Approach**: Use **arda-ingest API** for Phase 1 (simpler, lower latency), but design the interface abstractly so we can swap to **mcp-arda-vector-documents** if we need its richer toolset (covenant analysis, taxonomy queries) in later phases.

```rust
// Abstract trait allows swapping implementations
#[async_trait]
pub trait DocumentVectorService: Send + Sync {
    async fn semantic_search(
        &self,
        query: &str,
        company_id: Option<Uuid>,
        deal_id: Option<Uuid>,
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<DocumentChunk>>;
    
    async fn get_document_metadata(&self, document_id: Uuid) -> Result<DocumentMetadata>;
}

// Phase 1: Direct arda-ingest API
pub struct ArdaIngestClient { /* ... */ }

// Future: MCP-based client  
pub struct MCPVectorDocumentsClient { /* ... */ }
```

---

## 2. Revised Architecture

### 2.1 High-Level System Context

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ARDA ECOSYSTEM                                  │
│                                                                             │
│  ┌─────────────────────┐         Events          ┌─────────────────────┐   │
│  │                     │ ───────────────────────▶│                     │   │
│  │    arda-credit      │                         │  arda-collateral-   │   │
│  │    (System of       │◀─────────────────────── │  intel              │   │
│  │     Record)         │    Detection Results    │  (Intelligence      │   │
│  │                     │                         │   Layer)            │   │
│  │  • Collateral CRUD  │                         │                     │   │
│  │  • Deal lifecycle   │                         │  • Knowledge Graph  │   │
│  │  • User notifs      │                         │  • AI Detection     │   │
│  │                     │                         │  • Lifecycle Intel  │   │
│  └─────────┬───────────┘                         └──────────┬──────────┘   │
│            │                                                 │              │
│            │ REST API                                        │ REST API     │
│            ▼                                                 ▼              │
│  ┌─────────────────────┐                         ┌─────────────────────┐   │
│  │  PostgreSQL         │                         │  SurrealDB          │   │
│  │  (arda-credit DB)   │                         │  (collateral-intel) │   │
│  │                     │                         │                     │   │
│  │  Source of Truth:   │                         │  Derived Knowledge: │   │
│  │  • collateral       │                         │  • collateral_graph │   │
│  │  • deals            │                         │  • detection_history│   │
│  │  • deal_collateral  │                         │  • lifecycle_events │   │
│  └─────────────────────┘                         │  • embeddings_cache │   │
│                                                  └─────────────────────┘   │
│                                                             │              │
│                                                             │ API          │
│                                                             ▼              │
│                                                  ┌─────────────────────┐   │
│                                                  │  arda-ingest        │   │
│                                                  │  (Document Vectors) │   │
│                                                  │                     │   │
│                                                  │  • SurrealDB IDR    │   │
│                                                  │  • Document chunks  │   │
│                                                  │  • Embeddings       │   │
│                                                  └─────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Event Flow: arda-credit as "Dumb Publisher"

arda-credit publishes state changes without interpretation. arda-collateral-intel interprets meaning.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         EVENT FLOW                                          │
│                                                                             │
│  arda-credit (Publisher)              arda-collateral-intel (Consumer)      │
│  ─────────────────────────            ───────────────────────────────       │
│                                                                             │
│  Collateral Events:                   Interpretation:                       │
│  ├─ CollateralCreated ────────────▶  "New collateral, compare against all" │
│  ├─ CollateralUpdated ────────────▶  "Re-check, info may reveal matches"   │
│  └─ CollateralDeleted ────────────▶  "Remove from knowledge graph"         │
│                                                                             │
│  Deal Events:                         Interpretation:                       │
│  ├─ DealCreated ──────────────────▶  "Note: these collaterals now in use"  │
│  ├─ DealStatusChanged ────────────▶  See status interpretation below       │
│  │   ├─ → Active                     "Collaterals are IN USE"              │
│  │   ├─ → Finalized                  "Collaterals are NOT IN USE"          │
│  │   ├─ → PaidOff                    "Collaterals are NOT IN USE"          │
│  │   ├─ → Defaulted                  "Collaterals status unclear"          │
│  │   └─ → Cancelled                  "Collaterals are NOT IN USE"          │
│  └─ DealCollateralLinked ─────────▶  "Update graph: collateral→deal edge" │
│                                                                             │
│  Document Events:                     Interpretation:                       │
│  └─ DocumentUploaded ─────────────▶  "Search for collateral mentions"      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Collateral Lifecycle States

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COLLATERAL LIFECYCLE                                     │
│                                                                             │
│                        ┌──────────────┐                                     │
│                        │   Created    │                                     │
│                        │  (Unlinked)  │                                     │
│                        └──────┬───────┘                                     │
│                               │                                             │
│                    DealCollateralLinked                                     │
│                               │                                             │
│                               ▼                                             │
│  ┌───────────────────────────────────────────────────────────────────┐     │
│  │                                                                   │     │
│  │                     ┌──────────────┐                              │     │
│  │          ┌─────────▶│   IN USE     │◀─────────┐                   │     │
│  │          │          │  (Active     │          │                   │     │
│  │          │          │   Deal)      │          │                   │     │
│  │          │          └──────┬───────┘          │                   │     │
│  │          │                 │                  │                   │     │
│  │   New Deal           Deal Finalized      Deal Reactivated         │     │
│  │   Linked             / Paid Off          (rare)                   │     │
│  │          │                 │                  │                   │     │
│  │          │                 ▼                  │                   │     │
│  │          │          ┌──────────────┐          │                   │     │
│  │          └──────────│  NOT IN USE  │──────────┘                   │     │
│  │                     │  (Available) │                              │     │
│  │                     └──────────────┘                              │     │
│  │                                                                   │     │
│  │  Note: Same collateral can have multiple deals in history.        │     │
│  │  Only ONE active deal at a time = valid. Multiple active = ALERT! │     │
│  │                                                                   │     │
│  └───────────────────────────────────────────────────────────────────┘     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Internal Knowledge Graph (SurrealDB)

### 3.1 Why SurrealDB?

| Requirement | SurrealDB Capability |
|-------------|---------------------|
| Graph relationships (collateral→deal→company) | Native graph with `->` syntax |
| Vector embeddings for similarity search | Built-in vector index |
| Flexible schema evolution | Schemaless by default |
| Historical event storage | Time-series friendly |
| Consistency with arda-ingest | Same technology stack |

### 3.2 Schema Design

```sql
-- ============================================================================
-- COLLATERAL KNOWLEDGE GRAPH SCHEMA
-- ============================================================================

-- Namespace and database
USE NS arda DB collateral_intel;

-- ============================================================================
-- CORE ENTITIES
-- ============================================================================

-- Collateral Entity (derived from arda-credit events)
DEFINE TABLE collateral SCHEMAFULL;
DEFINE FIELD external_id ON collateral TYPE string;      -- UUID from arda-credit
DEFINE FIELD name ON collateral TYPE string;
DEFINE FIELD collateral_type ON collateral TYPE string;
DEFINE FIELD address ON collateral TYPE option<string>;
DEFINE FIELD custom_id ON collateral TYPE option<string>;
DEFINE FIELD description ON collateral TYPE option<string>;
DEFINE FIELD company_id ON collateral TYPE string;       -- UUID from arda-credit
DEFINE FIELD lifecycle_status ON collateral TYPE string; -- "in_use", "not_in_use", "unknown"
DEFINE FIELD active_deal_count ON collateral TYPE int DEFAULT 0;
DEFINE FIELD total_deal_count ON collateral TYPE int DEFAULT 0;
DEFINE FIELD embedding ON collateral TYPE option<array>; -- 4096-dim cached embedding
DEFINE FIELD embedding_updated_at ON collateral TYPE option<datetime>;
DEFINE FIELD created_at ON collateral TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON collateral TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_collateral_external ON collateral FIELDS external_id UNIQUE;
DEFINE INDEX idx_collateral_company ON collateral FIELDS company_id;
DEFINE INDEX idx_collateral_status ON collateral FIELDS lifecycle_status;

-- Deal Entity (minimal, just for relationship tracking)
DEFINE TABLE deal SCHEMAFULL;
DEFINE FIELD external_id ON deal TYPE string;           -- UUID from arda-credit
DEFINE FIELD status ON deal TYPE string;                -- "active", "finalized", "paid_off", etc.
DEFINE FIELD company_id ON deal TYPE string;
DEFINE FIELD created_at ON deal TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON deal TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_deal_external ON deal FIELDS external_id UNIQUE;
DEFINE INDEX idx_deal_status ON deal FIELDS status;

-- ============================================================================
-- RELATIONSHIPS (Graph Edges)
-- ============================================================================

-- Collateral → Deal relationship
DEFINE TABLE pledged_to SCHEMAFULL;
DEFINE FIELD in ON pledged_to TYPE record<collateral>;
DEFINE FIELD out ON pledged_to TYPE record<deal>;
DEFINE FIELD pledged_at ON pledged_to TYPE datetime;
DEFINE FIELD released_at ON pledged_to TYPE option<datetime>;
DEFINE FIELD is_active ON pledged_to TYPE bool DEFAULT true;

-- Collateral similarity relationship (detected matches)
DEFINE TABLE similar_to SCHEMAFULL;
DEFINE FIELD in ON similar_to TYPE record<collateral>;
DEFINE FIELD out ON similar_to TYPE record<collateral>;
DEFINE FIELD similarity_score ON similar_to TYPE float;
DEFINE FIELD match_signals ON similar_to TYPE array;    -- JSON array of signals
DEFINE FIELD detected_at ON similar_to TYPE datetime DEFAULT time::now();
DEFINE FIELD detection_version ON similar_to TYPE string;

-- ============================================================================
-- DETECTION RECORDS
-- ============================================================================

DEFINE TABLE detection SCHEMAFULL;
DEFINE FIELD source_collateral ON detection TYPE record<collateral>;
DEFINE FIELD trigger_type ON detection TYPE string;     -- "created", "updated", "manual_scan"
DEFINE FIELD status ON detection TYPE string;           -- "clear", "potential_match", "high_risk"
DEFINE FIELD highest_score ON detection TYPE option<float>;
DEFINE FIELD match_count ON detection TYPE int DEFAULT 0;
DEFINE FIELD analysis_duration_ms ON detection TYPE int;
DEFINE FIELD analysis_version ON detection TYPE string;
DEFINE FIELD created_at ON detection TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_detection_source ON detection FIELDS source_collateral;
DEFINE INDEX idx_detection_status ON detection FIELDS status;
DEFINE INDEX idx_detection_created ON detection FIELDS created_at;

-- Detection matches (child records)
DEFINE TABLE detection_match SCHEMAFULL;
DEFINE FIELD detection ON detection_match TYPE record<detection>;
DEFINE FIELD matched_collateral ON detection_match TYPE record<collateral>;
DEFINE FIELD overall_score ON detection_match TYPE float;
DEFINE FIELD confidence ON detection_match TYPE string; -- "high", "medium", "low"
DEFINE FIELD signals ON detection_match TYPE array;     -- Signal details
DEFINE FIELD is_same_deal ON detection_match TYPE bool; -- Same deal = not a problem
DEFINE FIELD both_deals_active ON detection_match TYPE bool; -- Both active = PROBLEM

-- ============================================================================
-- EVENT LOG (Audit Trail)
-- ============================================================================

DEFINE TABLE event_log SCHEMAFULL;
DEFINE FIELD event_type ON event_log TYPE string;
DEFINE FIELD source ON event_log TYPE string;           -- "arda-credit", "arda-ingest", "internal"
DEFINE FIELD entity_type ON event_log TYPE string;      -- "collateral", "deal"
DEFINE FIELD entity_id ON event_log TYPE string;
DEFINE FIELD payload ON event_log TYPE object;          -- Full event data
DEFINE FIELD processed_at ON event_log TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_event_type ON event_log FIELDS event_type;
DEFINE INDEX idx_event_entity ON event_log FIELDS entity_type, entity_id;

-- ============================================================================
-- EMBEDDING CACHE
-- ============================================================================

DEFINE TABLE embedding_cache SCHEMAFULL;
DEFINE FIELD content_hash ON embedding_cache TYPE string;  -- Hash of text content
DEFINE FIELD embedding ON embedding_cache TYPE array;      -- 4096-dim vector
DEFINE FIELD model_version ON embedding_cache TYPE string;
DEFINE FIELD created_at ON embedding_cache TYPE datetime DEFAULT time::now();
DEFINE INDEX idx_embedding_hash ON embedding_cache FIELDS content_hash UNIQUE;
```

### 3.3 Key Graph Queries

```sql
-- Find all active pledges for a collateral
SELECT 
    <-pledged_to<-collateral AS collateral,
    ->pledged_to->deal AS deals
FROM collateral
WHERE external_id = $collateral_id
AND ->pledged_to.is_active = true;

-- Find potentially double-pledged collaterals (multiple active deals)
SELECT 
    id,
    name,
    active_deal_count,
    ->pledged_to[WHERE is_active = true]->deal AS active_deals
FROM collateral
WHERE active_deal_count > 1;

-- Get similar collaterals with high similarity
SELECT 
    in AS source,
    out AS match,
    similarity_score,
    match_signals
FROM similar_to
WHERE in = $collateral_id
AND similarity_score >= 0.85
ORDER BY similarity_score DESC;

-- Get detection history for a collateral
SELECT 
    *,
    (SELECT * FROM detection_match WHERE detection = $parent.id) AS matches
FROM detection
WHERE source_collateral = $collateral_id
ORDER BY created_at DESC
LIMIT 10;
```

---

## 4. Service Components

### 4.1 API Contracts

#### Incoming: Events from arda-credit

**POST /api/v1/events/collateral**
```json
{
  "event_id": "uuid",
  "event_type": "CollateralCreated",
  "timestamp": "2025-12-04T10:30:00Z",
  "collateral": {
    "id": "uuid",
    "name": "123 Main Street Office Building",
    "collateral_type": "commercial_property",
    "address": "123 Main St, New York, NY 10001",
    "custom_id": "PROP-2025-001",
    "description": "20-story Class A office building",
    "company_id": "uuid"
  }
}
```

**POST /api/v1/events/deal**
```json
{
  "event_id": "uuid",
  "event_type": "DealStatusChanged",
  "timestamp": "2025-12-04T10:30:00Z",
  "deal": {
    "id": "uuid",
    "status": "finalized",
    "previous_status": "active",
    "company_id": "uuid",
    "collateral_ids": ["uuid1", "uuid2"]
  }
}
```

#### Outgoing: Detection Results to arda-credit

**POST {ARDA_CREDIT_API_URL}/api/v1/internal/collateral/{id}/detection**
```json
{
  "collateral_id": "uuid",
  "detection_status": "potential_match",
  "highest_match_score": 0.92,
  "match_count": 2,
  "matches": [
    {
      "matched_collateral_id": "uuid",
      "matched_deal_id": "uuid",
      "overall_score": 0.92,
      "confidence": "high",
      "is_same_deal": false,
      "both_deals_active": true,
      "summary": "High similarity with collateral in active deal - potential double pledge"
    }
  ],
  "analyzed_at": "2025-12-04T10:30:15Z"
}
```

#### Query: Detection Details

**GET /api/v1/collateral/{id}/detection-history**
```json
{
  "collateral_id": "uuid",
  "detections": [
    {
      "detection_id": "uuid",
      "trigger_type": "created",
      "status": "potential_match",
      "highest_score": 0.92,
      "match_count": 2,
      "analyzed_at": "2025-12-04T10:30:15Z",
      "matches": [
        {
          "matched_collateral_id": "uuid",
          "matched_collateral_name": "123 Main St Office Tower",
          "matched_deal_id": "uuid",
          "score": 0.92,
          "signals": [
            {
              "type": "AddressEmbeddingSimilarity",
              "score": 0.95,
              "evidence": "Addresses are semantically identical"
            },
            {
              "type": "SameDocumentMention", 
              "score": 0.88,
              "evidence": "Both mentioned in DOC-2025-100"
            }
          ],
          "is_concerning": true,
          "reason": "Both collaterals are pledged to active deals"
        }
      ]
    }
  ]
}
```

### 4.2 Core Service Structure

```rust
// Main service components
pub struct CollateralIntelService {
    // Own database
    db: Surreal<Any>,
    
    // External service clients (API-based)
    arda_credit_client: ArdaCreditApiClient,
    document_vector_client: Box<dyn DocumentVectorService>,
    embedding_client: ModalEmbeddingClient,
    
    // Internal components
    analysis_pipeline: AnalysisPipeline,
    event_processor: EventProcessor,
    knowledge_graph: KnowledgeGraphManager,
    
    config: ServiceConfig,
}

pub struct ServiceConfig {
    // Service
    pub port: u16,
    
    // External APIs (no direct DB access!)
    pub arda_credit_api_url: String,
    pub arda_ingest_api_url: String,
    
    // Own database
    pub surrealdb_url: String,
    pub surrealdb_namespace: String,
    pub surrealdb_database: String,
    pub surrealdb_username: String,
    pub surrealdb_password: String,
    
    // Modal embedding
    pub modal_embedding_endpoint: String,
    
    // Analysis config
    pub similarity_threshold: f32,
    pub max_concurrent_analyses: usize,
}
```

### 4.3 Analysis Pipeline

```rust
pub struct AnalysisPipeline {
    embedding_client: ModalEmbeddingClient,
    document_client: Box<dyn DocumentVectorService>,
    knowledge_graph: KnowledgeGraphManager,
}

impl AnalysisPipeline {
    /// Analyze a collateral for potential double-pledge
    pub async fn analyze_collateral(
        &self,
        collateral: &Collateral,
        trigger: AnalysisTrigger,
    ) -> Result<DetectionResult> {
        let start = Instant::now();
        
        // Step 1: Generate/retrieve embedding for this collateral
        let embedding = self.get_or_generate_embedding(collateral).await?;
        
        // Step 2: Find similar collaterals in knowledge graph
        let similar_collaterals = self.knowledge_graph
            .find_similar_collaterals(&embedding, 0.7)
            .await?;
        
        // Step 3: Enrich with document cross-references
        let document_matches = self.document_client
            .semantic_search(
                &collateral.searchable_text(),
                Some(collateral.company_id),
                None,  // Search across all deals
                20,
                0.6,
            )
            .await?;
        
        // Step 4: Aggregate signals and compute scores
        let matches = self.compute_matches(
            collateral,
            similar_collaterals,
            document_matches,
        ).await?;
        
        // Step 5: Determine if concerning (same collateral in multiple ACTIVE deals)
        let concerning_matches: Vec<_> = matches.iter()
            .filter(|m| m.both_deals_active && !m.is_same_deal)
            .collect();
        
        let status = match concerning_matches.len() {
            0 => DetectionStatus::Clear,
            1..=2 => DetectionStatus::PotentialMatch,
            _ => DetectionStatus::HighRisk,
        };
        
        Ok(DetectionResult {
            source_collateral_id: collateral.id,
            trigger,
            status,
            highest_score: matches.first().map(|m| m.overall_score),
            match_count: matches.len(),
            matches,
            analysis_duration_ms: start.elapsed().as_millis() as u64,
            analysis_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}
```

---

## 5. Events Required from arda-credit

### 5.1 Event Catalog

| Event | Trigger | Payload | Intel Service Action |
|-------|---------|---------|---------------------|
| `CollateralCreated` | POST /collateral | Full collateral object | Add to graph, generate embedding, compare against all |
| `CollateralUpdated` | PUT /collateral/{id} | Full collateral object + changes | Update graph, regenerate embedding if text changed, re-compare |
| `CollateralDeleted` | DELETE /collateral/{id} | Collateral ID | Remove from graph, remove similarity edges |
| `DealCreated` | POST /deals | Deal + collateral IDs | Add deal to graph, create pledged_to edges |
| `DealStatusChanged` | PUT /deals/{id}/status | Deal ID, new status, old status, collateral IDs | Update deal status, recalculate collateral lifecycle |
| `DealCollateralLinked` | POST /deals/{id}/collateral | Deal ID, collateral IDs | Create pledged_to edge, mark collateral as in_use |
| `DealCollateralUnlinked` | DELETE /deals/{id}/collateral/{cid} | Deal ID, collateral ID | Remove edge, recalculate if any active deals remain |

### 5.2 arda-credit Implementation Requirements

```rust
// New module: api/src/events/collateral_events.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum CollateralEvent {
    CollateralCreated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral: CollateralPayload,
    },
    CollateralUpdated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral: CollateralPayload,
        changes: Vec<FieldChange>,
    },
    CollateralDeleted {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral_id: Uuid,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DealEvent {
    DealCreated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal: DealPayload,
        collateral_ids: Vec<Uuid>,
    },
    DealStatusChanged {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        previous_status: DealStatus,
        new_status: DealStatus,
        collateral_ids: Vec<Uuid>,
    },
    DealCollateralLinked {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        collateral_ids: Vec<Uuid>,
    },
    DealCollateralUnlinked {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        collateral_id: Uuid,
    },
}

// Event publisher service
pub struct CollateralEventPublisher {
    intel_service_url: String,
    client: reqwest::Client,
}

impl CollateralEventPublisher {
    pub async fn publish(&self, event: impl Serialize) -> Result<(), PublishError> {
        let url = format!("{}/api/v1/events", self.intel_service_url);
        self.client.post(&url)
            .json(&event)
            .send()
            .await?;
        Ok(())
    }
}
```

---

## 6. Environment Configuration

### 6.1 Environment Variables

```env
# =============================================================================
# SERVICE CONFIGURATION
# =============================================================================
PORT=3010
RUST_LOG=info,arda_collateral_intel=debug

# =============================================================================
# EXTERNAL APIS (No direct DB access!)
# =============================================================================
ARDA_CREDIT_API_URL=http://arda-credit:8080
ARDA_INGEST_API_URL=http://arda-ingest:3000

# =============================================================================
# OWN DATABASE (Collateral Intel's SurrealDB)
# =============================================================================
SURREALDB_URL=ws://surrealdb-collateral-intel:8000
SURREALDB_NAMESPACE=arda
SURREALDB_DATABASE=collateral_intel
SURREALDB_USERNAME=root
SURREALDB_PASSWORD=secret

# =============================================================================
# EMBEDDING SERVICE
# =============================================================================
MODAL_EMBEDDING_ENDPOINT=https://ardaglobal--embed.modal.run

# =============================================================================
# ANALYSIS CONFIGURATION
# =============================================================================
SIMILARITY_THRESHOLD=0.85
MAX_CONCURRENT_ANALYSES=10
EMBEDDING_CACHE_TTL_DAYS=30
```

### 6.2 Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: arda-collateral-intel
  namespace: arda
spec:
  replicas: 2
  selector:
    matchLabels:
      app: arda-collateral-intel
  template:
    metadata:
      labels:
        app: arda-collateral-intel
    spec:
      containers:
        - name: arda-collateral-intel
          image: ghcr.io/ardaglobal/arda-collateral-intel:latest
          ports:
            - containerPort: 3010
          envFrom:
            - configMapRef:
                name: arda-collateral-intel-config
            - secretRef:
                name: arda-collateral-intel-secrets
          resources:
            requests:
              memory: "256Mi"
              cpu: "100m"
            limits:
              memory: "512Mi"
              cpu: "500m"
          livenessProbe:
            httpGet:
              path: /health
              port: 3010
            initialDelaySeconds: 10
            periodSeconds: 30
          readinessProbe:
            httpGet:
              path: /ready
              port: 3010
            initialDelaySeconds: 5
            periodSeconds: 10

---
apiVersion: v1
kind: Service
metadata:
  name: arda-collateral-intel
  namespace: arda
spec:
  type: ClusterIP
  selector:
    app: arda-collateral-intel
  ports:
    - port: 3010
      targetPort: 3010

---
# Own SurrealDB instance for the intel service
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: surrealdb-collateral-intel
  namespace: arda
spec:
  serviceName: surrealdb-collateral-intel
  replicas: 1
  selector:
    matchLabels:
      app: surrealdb-collateral-intel
  template:
    metadata:
      labels:
        app: surrealdb-collateral-intel
    spec:
      containers:
        - name: surrealdb
          image: surrealdb/surrealdb:latest
          args:
            - start
            - --user=root
            - --pass=$(SURREALDB_PASSWORD)
            - file:/data/database.db
          ports:
            - containerPort: 8000
          volumeMounts:
            - name: data
              mountPath: /data
          envFrom:
            - secretRef:
                name: arda-collateral-intel-secrets
  volumeClaimTemplates:
    - metadata:
        name: data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 10Gi
```

---

## 7. Implementation Phases

### Phase 1: Foundation (2-3 weeks)

**Goals**: Service scaffold with event processing and basic detection

**Deliverables**:
- [ ] Rust project scaffold with Axum
- [ ] SurrealDB schema and connection
- [ ] Event webhook endpoints
- [ ] Knowledge graph basic CRUD
- [ ] Embedding generation and caching
- [ ] Basic "compare against all" detection
- [ ] Detection result publishing to arda-credit
- [ ] Docker + Helm charts

### Phase 2: Deal Lifecycle Intelligence (2 weeks)

**Goals**: Understand deal status and collateral availability

**Deliverables**:
- [ ] Deal status event processing
- [ ] Collateral lifecycle state machine
- [ ] "In use" vs "Not in use" tracking
- [ ] Smart detection: only alert if BOTH deals active
- [ ] Historical deal analysis for collateral

### Phase 3: Document Cross-Reference (2 weeks)

**Goals**: Integrate document vector search

**Deliverables**:
- [ ] arda-ingest API client (or MCP client)
- [ ] Document-based similarity signals
- [ ] Security agreement cross-referencing
- [ ] Enhanced match scoring with document evidence

### Phase 4: Knowledge Graph Enhancement (1-2 weeks)

**Goals**: Build learning and optimization

**Deliverables**:
- [ ] Embedding cache optimization
- [ ] Similarity index for faster lookups
- [ ] Detection pattern analysis
- [ ] False positive feedback loop (future)

---

## 8. Testing Strategy

### Integration Tests

```rust
#[tokio::test]
async fn test_collateral_created_triggers_analysis() {
    let service = TestService::new().await;
    
    // Simulate CollateralCreated event
    let event = CollateralEvent::CollateralCreated {
        collateral: Collateral {
            id: Uuid::new_v4(),
            name: "123 Main Street Office".to_string(),
            ..Default::default()
        },
    };
    
    service.process_event(event).await.unwrap();
    
    // Verify collateral added to knowledge graph
    let collateral = service.knowledge_graph
        .get_collateral(event.collateral.id)
        .await
        .unwrap();
    assert!(collateral.is_some());
    
    // Verify detection was run (even if no matches found)
    let detections = service.knowledge_graph
        .get_detection_history(event.collateral.id)
        .await
        .unwrap();
    assert_eq!(detections.len(), 1);
    assert_eq!(detections[0].status, DetectionStatus::Clear);
}

#[tokio::test]
async fn test_double_pledge_detected_when_both_deals_active() {
    let service = TestService::new().await;
    
    // Create first collateral with active deal
    let collateral_1 = create_collateral_with_active_deal(&service).await;
    
    // Create similar collateral with ANOTHER active deal
    let collateral_2 = Collateral {
        id: Uuid::new_v4(),
        name: "123 Main St Office Building".to_string(), // Very similar!
        address: Some("123 Main Street, New York".to_string()),
        ..Default::default()
    };
    
    let result = service.analyze_collateral(&collateral_2).await.unwrap();
    
    // Should detect potential double pledge
    assert_eq!(result.status, DetectionStatus::PotentialMatch);
    assert!(result.matches[0].both_deals_active);
}

#[tokio::test]
async fn test_no_alert_when_previous_deal_finalized() {
    let service = TestService::new().await;
    
    // Create collateral with FINALIZED deal
    let collateral_1 = create_collateral_with_finalized_deal(&service).await;
    
    // Create similar collateral with new active deal
    let collateral_2 = Collateral {
        id: Uuid::new_v4(),
        name: "123 Main St Office Building".to_string(),
        ..Default::default()
    };
    
    let result = service.analyze_collateral(&collateral_2).await.unwrap();
    
    // Should NOT alert because previous deal is finalized
    assert_eq!(result.status, DetectionStatus::Clear);
    // Or if matches found, they should not be flagged as concerning
    if !result.matches.is_empty() {
        assert!(!result.matches[0].both_deals_active);
    }
}
```

---

## 9. Open Questions Resolved

| Question | Resolution |
|----------|------------|
| Embedding caching? | Yes, cache in own SurrealDB with TTL |
| Lookup strategy? | Start with "compare all", optimize later with vector index |
| User notification? | Not our concern; publish to arda-credit, they handle UX |
| Resolution workflow? | Store detection details; expose query API for details |
| Historical backfill? | Not our concern; arda-credit triggers events as needed |

---

## 10. Future Considerations

1. **Vector Index Optimization**: When collateral count grows, add HNSW index for O(log n) similarity lookup

2. **MCP Integration**: If we need covenant analysis or richer document tools, swap to mcp-arda-vector-documents

3. **Claude Reasoning**: For ambiguous cases, add Claude API for natural language analysis

4. **Feedback Loop**: Allow users to mark false positives; train detection thresholds

5. **Cross-Company Analysis**: Future capability to detect collateral pledged across different companies

---

## Appendix A: Sequence Diagrams

### A.1 Collateral Created Flow

```
┌─────────┐     ┌─────────────┐     ┌───────────────────┐     ┌────────────┐
│  User   │     │ arda-credit │     │ arda-collateral-  │     │ arda-ingest│
│         │     │             │     │ intel             │     │            │
└────┬────┘     └──────┬──────┘     └─────────┬─────────┘     └─────┬──────┘
     │                 │                      │                      │
     │ POST /collateral│                      │                      │
     │────────────────▶│                      │                      │
     │                 │                      │                      │
     │                 │ Save to PostgreSQL   │                      │
     │                 │──────────┐           │                      │
     │                 │          │           │                      │
     │                 │◀─────────┘           │                      │
     │                 │                      │                      │
     │                 │ POST /events/collateral                     │
     │                 │ (CollateralCreated)  │                      │
     │                 │─────────────────────▶│                      │
     │                 │                      │                      │
     │                 │                      │ Add to knowledge graph
     │                 │                      │──────────┐           │
     │                 │                      │          │           │
     │                 │                      │◀─────────┘           │
     │                 │                      │                      │
     │                 │                      │ Generate embedding   │
     │                 │                      │──────────┐           │
     │                 │                      │          │           │
     │                 │                      │◀─────────┘           │
     │                 │                      │                      │
     │                 │                      │ GET /semantic-search │
     │                 │                      │ (document mentions)  │
     │                 │                      │─────────────────────▶│
     │                 │                      │                      │
     │                 │                      │◀─────────────────────│
     │                 │                      │                      │
     │                 │                      │ Compare against all  │
     │                 │                      │ collaterals in graph │
     │                 │                      │──────────┐           │
     │                 │                      │          │           │
     │                 │                      │◀─────────┘           │
     │                 │                      │                      │
     │                 │ POST /internal/      │                      │
     │                 │ collateral/{id}/     │                      │
     │                 │ detection            │                      │
     │                 │◀─────────────────────│                      │
     │                 │                      │                      │
     │  201 Created    │                      │                      │
     │◀────────────────│                      │                      │
     │                 │                      │                      │
```

---

## Appendix B: Comparison Matrix

| Aspect | Direct DB Access | API-Based (Recommended) |
|--------|-----------------|------------------------|
| Coupling | Tight | Loose |
| Schema changes | Breaks service | Versioned API contracts |
| Testing | Requires shared DB | Mock API responses |
| Deployment | Must deploy together | Independent |
| Security | DB credentials shared | API-level auth |
| Scalability | DB connection limits | API rate limiting |
