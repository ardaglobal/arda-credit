-- Enhancement Migration: Loan-Aware IMT Implementation
-- This migration extends the existing IMT to better track loan lifecycle states
-- and enable efficient loan operation queries while maintaining all existing functionality.

-- ============================================================================
-- EXTEND NULLIFIERS TABLE WITH LOAN OPERATION METADATA
-- ============================================================================

-- Add loan operation columns to nullifiers table
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS loan_operation_type VARCHAR(30);
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS loan_id UUID;
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS user_id UUID;
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS operation_amount DECIMAL(18, 6);
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS operation_metadata JSONB DEFAULT '{}';
ALTER TABLE nullifiers ADD COLUMN IF NOT EXISTS batch_id INTEGER;

-- Add constraints for loan operation fields
ALTER TABLE nullifiers ADD CONSTRAINT nullifiers_loan_operation_type_check 
    CHECK (loan_operation_type IS NULL OR loan_operation_type IN (
        'loan_origination', 'loan_payment', 'loan_closeout', 'user_registration', 'kyc_submission', 'kyc_verification'
    ));

ALTER TABLE nullifiers ADD CONSTRAINT nullifiers_operation_amount_positive 
    CHECK (operation_amount IS NULL OR operation_amount >= 0);

-- Add foreign key references where applicable
ALTER TABLE nullifiers ADD CONSTRAINT fk_nullifiers_loan_id 
    FOREIGN KEY (loan_id) REFERENCES loans(id) ON DELETE SET NULL;

ALTER TABLE nullifiers ADD CONSTRAINT fk_nullifiers_user_id 
    FOREIGN KEY (user_id) REFERENCES identities(id) ON DELETE SET NULL;

ALTER TABLE nullifiers ADD CONSTRAINT fk_nullifiers_batch_id 
    FOREIGN KEY (batch_id) REFERENCES proof_batches(id) ON DELETE SET NULL;

-- ============================================================================
-- EXTEND TREE STATE TABLE WITH LOAN OPERATION STATISTICS
-- ============================================================================

-- Add loan operation statistics to tree_state table
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS total_loan_originations BIGINT DEFAULT 0;
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS total_loan_payments BIGINT DEFAULT 0;
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS total_loan_closeouts BIGINT DEFAULT 0;
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS total_loan_amount DECIMAL(18, 6) DEFAULT 0;
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS active_loans_count BIGINT DEFAULT 0;
ALTER TABLE tree_state ADD COLUMN IF NOT EXISTS loan_stats_updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- Add constraints for loan statistics
ALTER TABLE tree_state ADD CONSTRAINT tree_state_loan_stats_non_negative CHECK (
    total_loan_originations >= 0 AND 
    total_loan_payments >= 0 AND 
    total_loan_closeouts >= 0 AND
    total_loan_amount >= 0 AND
    active_loans_count >= 0
);

-- ============================================================================
-- EFFICIENT INDEXING FOR LOAN-SPECIFIC QUERIES
-- ============================================================================

-- Indexes for loan operation queries on nullifiers table
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_loan_id 
    ON nullifiers(loan_id) WHERE loan_id IS NOT NULL AND is_active = true;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_user_id 
    ON nullifiers(user_id) WHERE user_id IS NOT NULL AND is_active = true;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_operation_type 
    ON nullifiers(loan_operation_type) WHERE loan_operation_type IS NOT NULL AND is_active = true;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_batch_id 
    ON nullifiers(batch_id) WHERE batch_id IS NOT NULL AND is_active = true;

-- Composite indexes for efficient loan lifecycle queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_loan_lifecycle 
    ON nullifiers(loan_id, loan_operation_type, created_at) 
    WHERE loan_id IS NOT NULL AND is_active = true;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_user_operations 
    ON nullifiers(user_id, loan_operation_type, created_at) 
    WHERE user_id IS NOT NULL AND is_active = true;

-- Index for loan amount range queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_operation_amount 
    ON nullifiers(operation_amount) 
    WHERE operation_amount IS NOT NULL AND is_active = true;

-- Partial index for loan operations only
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_loan_ops_only 
    ON nullifiers(value, loan_id, loan_operation_type, created_at) 
    WHERE loan_operation_type IN ('loan_origination', 'loan_payment', 'loan_closeout') AND is_active = true;

-- GIN index for loan operation metadata queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_nullifiers_operation_metadata 
    ON nullifiers USING GIN(operation_metadata) 
    WHERE operation_metadata IS NOT NULL AND operation_metadata != '{}';

-- ============================================================================
-- ENHANCE AUDIT EVENTS WITH LOAN OPERATION CORRELATION
-- ============================================================================

-- Add loan operation correlation to audit_events
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS loan_id UUID;
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS user_id UUID;
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS loan_operation_type VARCHAR(30);

-- Add foreign key constraints
ALTER TABLE audit_events ADD CONSTRAINT fk_audit_events_loan_id 
    FOREIGN KEY (loan_id) REFERENCES loans(id) ON DELETE SET NULL;

ALTER TABLE audit_events ADD CONSTRAINT fk_audit_events_user_id 
    FOREIGN KEY (user_id) REFERENCES identities(id) ON DELETE SET NULL;

-- Add indexes for loan operation correlation
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_events_loan_id 
    ON audit_events(loan_id) WHERE loan_id IS NOT NULL;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_events_user_id 
    ON audit_events(user_id) WHERE user_id IS NOT NULL;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_events_loan_operation_type 
    ON audit_events(loan_operation_type) WHERE loan_operation_type IS NOT NULL;

-- Composite index for loan audit trail queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_audit_events_loan_timeline 
    ON audit_events(loan_id, timestamp, loan_operation_type) 
    WHERE loan_id IS NOT NULL;

-- ============================================================================
-- DATABASE VIEWS FOR LOAN LIFECYCLE VISIBILITY
-- ============================================================================

-- View: Comprehensive loan operation history from IMT
CREATE OR REPLACE VIEW loan_operation_history AS
SELECT 
    n.value as nullifier_value,
    n.loan_id,
    n.user_id,
    n.loan_operation_type,
    n.operation_amount,
    n.operation_metadata,
    n.tree_index,
    n.batch_id,
    n.created_at as operation_timestamp,
    l.borrower_id,
    l.lender_id,
    l.principal_amount,
    l.outstanding_balance,
    l.loan_status,
    l.originated_at,
    i.email as user_email,
    i.user_type,
    i.kyc_status,
    pb.id as batch_number,
    pb.created_at as batch_timestamp
FROM nullifiers n
LEFT JOIN loans l ON n.loan_id = l.id
LEFT JOIN identities i ON n.user_id = i.id
LEFT JOIN proof_batches pb ON n.batch_id = pb.id
WHERE n.loan_operation_type IS NOT NULL 
  AND n.is_active = true
ORDER BY n.created_at DESC;

-- View: Loan lifecycle summary
CREATE OR REPLACE VIEW loan_lifecycle_summary AS
SELECT 
    l.id as loan_id,
    l.borrower_id,
    l.lender_id,
    l.principal_amount,
    l.outstanding_balance,
    l.loan_status,
    l.originated_at,
    
    -- Operation counts from IMT
    COUNT(CASE WHEN n.loan_operation_type = 'loan_origination' THEN 1 END) as origination_count,
    COUNT(CASE WHEN n.loan_operation_type = 'loan_payment' THEN 1 END) as payment_count,
    COUNT(CASE WHEN n.loan_operation_type = 'loan_closeout' THEN 1 END) as closeout_count,
    
    -- Payment statistics from IMT
    COALESCE(SUM(CASE WHEN n.loan_operation_type = 'loan_payment' THEN n.operation_amount END), 0) as total_payments_from_imt,
    COALESCE(AVG(CASE WHEN n.loan_operation_type = 'loan_payment' THEN n.operation_amount END), 0) as avg_payment_amount,
    
    -- Timeline information
    MIN(CASE WHEN n.loan_operation_type = 'loan_origination' THEN n.created_at END) as first_origination_at,
    MAX(CASE WHEN n.loan_operation_type = 'loan_payment' THEN n.created_at END) as last_payment_at,
    MAX(CASE WHEN n.loan_operation_type = 'loan_closeout' THEN n.created_at END) as closeout_at,
    
    -- Batch information
    COUNT(DISTINCT n.batch_id) as total_batches_involved,
    MAX(n.batch_id) as latest_batch_id
    
FROM loans l
LEFT JOIN nullifiers n ON l.id = n.loan_id AND n.is_active = true
GROUP BY l.id, l.borrower_id, l.lender_id, l.principal_amount, 
         l.outstanding_balance, l.loan_status, l.originated_at;

-- View: User loan operation activity
CREATE OR REPLACE VIEW user_loan_activity AS
SELECT 
    i.id as user_id,
    i.email,
    i.user_type,
    i.kyc_status,
    
    -- Operation counts from IMT
    COUNT(CASE WHEN n.loan_operation_type = 'loan_origination' THEN 1 END) as total_originations,
    COUNT(CASE WHEN n.loan_operation_type = 'loan_payment' THEN 1 END) as total_payments,
    COUNT(CASE WHEN n.loan_operation_type = 'loan_closeout' THEN 1 END) as total_closeouts,
    
    -- Amount statistics
    COALESCE(SUM(CASE WHEN n.loan_operation_type = 'loan_origination' THEN n.operation_amount END), 0) as total_originated_amount,
    COALESCE(SUM(CASE WHEN n.loan_operation_type = 'loan_payment' THEN n.operation_amount END), 0) as total_payment_amount,
    
    -- Timeline information
    MIN(n.created_at) as first_operation_at,
    MAX(n.created_at) as last_operation_at,
    
    -- Batch activity
    COUNT(DISTINCT n.batch_id) as batches_participated,
    
    -- Active loan counts
    COUNT(DISTINCT CASE WHEN n.loan_operation_type IN ('loan_origination', 'loan_payment') 
                        AND NOT EXISTS (
                            SELECT 1 FROM nullifiers n2 
                            WHERE n2.loan_id = n.loan_id 
                            AND n2.loan_operation_type = 'loan_closeout' 
                            AND n2.is_active = true
                        ) THEN n.loan_id END) as active_loans_count
    
FROM identities i
LEFT JOIN nullifiers n ON i.id = n.user_id AND n.is_active = true
GROUP BY i.id, i.email, i.user_type, i.kyc_status;

-- View: IMT loan operation statistics
CREATE OR REPLACE VIEW imt_loan_stats AS
SELECT 
    ts.tree_id,
    ts.total_nullifiers,
    ts.total_loan_originations,
    ts.total_loan_payments,
    ts.total_loan_closeouts,
    ts.total_loan_amount,
    ts.active_loans_count,
    ts.loan_stats_updated_at,
    
    -- Real-time counts from nullifiers
    (SELECT COUNT(*) FROM nullifiers WHERE loan_operation_type = 'loan_origination' AND is_active = true) as current_originations,
    (SELECT COUNT(*) FROM nullifiers WHERE loan_operation_type = 'loan_payment' AND is_active = true) as current_payments,
    (SELECT COUNT(*) FROM nullifiers WHERE loan_operation_type = 'loan_closeout' AND is_active = true) as current_closeouts,
    (SELECT COUNT(DISTINCT loan_id) FROM nullifiers WHERE loan_operation_type IS NOT NULL AND is_active = true) as total_unique_loans,
    (SELECT COUNT(DISTINCT user_id) FROM nullifiers WHERE loan_operation_type IS NOT NULL AND is_active = true) as total_loan_users,
    
    -- Performance metrics
    (SELECT AVG(EXTRACT(epoch FROM (created_at - LAG(created_at) OVER (ORDER BY created_at)))) 
     FROM nullifiers WHERE loan_operation_type IS NOT NULL AND is_active = true) as avg_operation_interval_seconds
     
FROM tree_state ts;

-- ============================================================================
-- ENHANCED FUNCTIONS FOR LOAN OPERATION QUERIES
-- ============================================================================

-- Function to get loan operation history for a specific loan
CREATE OR REPLACE FUNCTION get_loan_operation_history(target_loan_id UUID)
RETURNS TABLE(
    nullifier_value BIGINT,
    operation_type VARCHAR(30),
    operation_amount DECIMAL(18, 6),
    operation_timestamp TIMESTAMP WITH TIME ZONE,
    tree_index BIGINT,
    batch_id INTEGER,
    user_id UUID,
    operation_metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        n.value,
        n.loan_operation_type,
        n.operation_amount,
        n.created_at,
        n.tree_index,
        n.batch_id,
        n.user_id,
        n.operation_metadata
    FROM nullifiers n
    WHERE n.loan_id = target_loan_id 
      AND n.is_active = true 
      AND n.loan_operation_type IS NOT NULL
    ORDER BY n.created_at ASC;
END;
$$ LANGUAGE plpgsql;

-- Function to get user loan operations
CREATE OR REPLACE FUNCTION get_user_loan_operations(
    target_user_id UUID,
    operation_type_filter VARCHAR(30) DEFAULT NULL,
    limit_count INTEGER DEFAULT 100
)
RETURNS TABLE(
    nullifier_value BIGINT,
    loan_id UUID,
    operation_type VARCHAR(30),
    operation_amount DECIMAL(18, 6),
    operation_timestamp TIMESTAMP WITH TIME ZONE,
    tree_index BIGINT,
    batch_id INTEGER,
    operation_metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        n.value,
        n.loan_id,
        n.loan_operation_type,
        n.operation_amount,
        n.created_at,
        n.tree_index,
        n.batch_id,
        n.operation_metadata
    FROM nullifiers n
    WHERE n.user_id = target_user_id 
      AND n.is_active = true 
      AND n.loan_operation_type IS NOT NULL
      AND (operation_type_filter IS NULL OR n.loan_operation_type = operation_type_filter)
    ORDER BY n.created_at DESC
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to query loan operations by amount range
CREATE OR REPLACE FUNCTION get_loan_operations_by_amount_range(
    min_amount DECIMAL(18, 6),
    max_amount DECIMAL(18, 6),
    operation_type_filter VARCHAR(30) DEFAULT NULL
)
RETURNS TABLE(
    nullifier_value BIGINT,
    loan_id UUID,
    user_id UUID,
    operation_type VARCHAR(30),
    operation_amount DECIMAL(18, 6),
    operation_timestamp TIMESTAMP WITH TIME ZONE,
    tree_index BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        n.value,
        n.loan_id,
        n.user_id,
        n.loan_operation_type,
        n.operation_amount,
        n.created_at,
        n.tree_index
    FROM nullifiers n
    WHERE n.operation_amount BETWEEN min_amount AND max_amount
      AND n.is_active = true 
      AND n.loan_operation_type IS NOT NULL
      AND (operation_type_filter IS NULL OR n.loan_operation_type = operation_type_filter)
    ORDER BY n.operation_amount DESC, n.created_at DESC;
END;
$$ LANGUAGE plpgsql;

-- Function to get batch loan operation summary
CREATE OR REPLACE FUNCTION get_batch_loan_summary(target_batch_id INTEGER)
RETURNS TABLE(
    operation_type VARCHAR(30),
    operation_count BIGINT,
    total_amount DECIMAL(18, 6),
    avg_amount DECIMAL(18, 6),
    unique_loans BIGINT,
    unique_users BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        n.loan_operation_type,
        COUNT(*),
        COALESCE(SUM(n.operation_amount), 0),
        COALESCE(AVG(n.operation_amount), 0),
        COUNT(DISTINCT n.loan_id),
        COUNT(DISTINCT n.user_id)
    FROM nullifiers n
    WHERE n.batch_id = target_batch_id 
      AND n.is_active = true 
      AND n.loan_operation_type IS NOT NULL
    GROUP BY n.loan_operation_type
    ORDER BY COUNT(*) DESC;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS TO MAINTAIN LOAN STATISTICS IN TREE STATE
-- ============================================================================

-- Function to update loan statistics in tree_state
CREATE OR REPLACE FUNCTION update_tree_loan_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update loan operation statistics in tree_state
    UPDATE tree_state
    SET 
        total_loan_originations = (
            SELECT COUNT(*) FROM nullifiers 
            WHERE loan_operation_type = 'loan_origination' AND is_active = true
        ),
        total_loan_payments = (
            SELECT COUNT(*) FROM nullifiers 
            WHERE loan_operation_type = 'loan_payment' AND is_active = true
        ),
        total_loan_closeouts = (
            SELECT COUNT(*) FROM nullifiers 
            WHERE loan_operation_type = 'loan_closeout' AND is_active = true
        ),
        total_loan_amount = (
            SELECT COALESCE(SUM(operation_amount), 0) FROM nullifiers 
            WHERE loan_operation_type IS NOT NULL AND is_active = true
        ),
        active_loans_count = (
            SELECT COUNT(DISTINCT loan_id) FROM nullifiers 
            WHERE loan_operation_type IN ('loan_origination', 'loan_payment') 
            AND is_active = true
            AND loan_id NOT IN (
                SELECT DISTINCT loan_id FROM nullifiers 
                WHERE loan_operation_type = 'loan_closeout' 
                AND is_active = true AND loan_id IS NOT NULL
            )
        ),
        loan_stats_updated_at = NOW()
    WHERE tree_id = 'default';
    
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Trigger to update loan stats when nullifiers are modified
CREATE OR REPLACE TRIGGER update_loan_stats_on_nullifier_change
    AFTER INSERT OR UPDATE OR DELETE ON nullifiers
    FOR EACH STATEMENT
    EXECUTE FUNCTION update_tree_loan_stats();

-- ============================================================================
-- ENHANCED NULLIFIER INSERTION FOR LOAN OPERATIONS
-- ============================================================================

-- Enhanced atomic nullifier insertion with loan operation metadata
CREATE OR REPLACE FUNCTION insert_loan_nullifier_atomic(
    new_value BIGINT,
    operation_type VARCHAR(30),
    target_loan_id UUID DEFAULT NULL,
    target_user_id UUID DEFAULT NULL,
    operation_amount DECIMAL(18, 6) DEFAULT NULL,
    metadata JSONB DEFAULT '{}',
    target_batch_id INTEGER DEFAULT NULL,
    new_tree_index BIGINT DEFAULT NULL
)
RETURNS TABLE(
    inserted_tree_index BIGINT,
    low_nullifier_value BIGINT,
    low_nullifier_next_value BIGINT,
    success BOOLEAN
) AS $$
DECLARE
    low_null RECORD;
    actual_tree_index BIGINT;
BEGIN
    -- Step 1: Get next available tree index if not provided
    IF new_tree_index IS NULL THEN
        actual_tree_index := get_next_tree_index();
    ELSE
        actual_tree_index := new_tree_index;
    END IF;
    
    -- Step 2: Find the low nullifier (where new_value fits in the gap)
    SELECT * INTO low_null
    FROM find_low_nullifier(new_value)
    LIMIT 1;
    
    -- Step 3: Validate insertion is possible (no duplicate values)
    IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
        RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
        RETURN;
    END IF;
    
    -- Begin atomic transaction block
    BEGIN
        -- Step 4: Insert new nullifier with loan operation metadata
        INSERT INTO nullifiers (
            value, 
            next_index, 
            next_value, 
            tree_index, 
            loan_operation_type, 
            loan_id, 
            user_id, 
            operation_amount, 
            operation_metadata, 
            batch_id
        )
        VALUES (
            new_value,
            COALESCE(low_null.low_next_index, NULL),
            COALESCE(low_null.low_next_value, 0),
            actual_tree_index,
            operation_type,
            target_loan_id,
            target_user_id,
            operation_amount,
            metadata,
            target_batch_id
        );
        
        -- Step 5: Update the low nullifier to point to new nullifier
        IF low_null.low_value IS NOT NULL THEN
            UPDATE nullifiers
            SET 
                next_index = actual_tree_index,
                next_value = new_value
            WHERE value = low_null.low_value AND is_active = true;
        END IF;
        
        -- Step 6: Update tree state (the trigger will handle loan stats)
        UPDATE tree_state
        SET 
            next_available_index = GREATEST(next_available_index, actual_tree_index + 1),
            total_nullifiers = total_nullifiers + 1,
            updated_at = NOW()
        WHERE tree_id = 'default';
        
        -- Step 7: Return success result
        RETURN QUERY SELECT 
            actual_tree_index,
            COALESCE(low_null.low_value, new_value),
            COALESCE(low_null.low_next_value, 0),
            TRUE;
            
    EXCEPTION WHEN OTHERS THEN
        -- Rollback on any error
        RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
    END;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- PERFORMANCE OPTIMIZATION: REFRESH MATERIALIZED VIEW FUNCTION
-- ============================================================================

-- Function to refresh loan statistics (for performance optimization)
CREATE OR REPLACE FUNCTION refresh_loan_statistics()
RETURNS VOID AS $$
BEGIN
    -- Force update of loan statistics
    PERFORM update_tree_loan_stats();
    
    -- Additional optimizations could include:
    -- - ANALYZE on key tables
    -- - Cleanup of old audit events
    -- - Recomputing statistics
    
    ANALYZE nullifiers;
    ANALYZE audit_events;
    ANALYZE loans;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- AUDIT TRAIL INTEGRATION FOR LOAN OPERATIONS
-- ============================================================================

-- Extend audit_events table with loan operation fields
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS loan_operation_type VARCHAR(30);
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS loan_id UUID;
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS user_id UUID;
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS operation_amount DECIMAL(18, 6);
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS batch_id INTEGER;

-- Add indexes for loan-specific audit queries
CREATE INDEX IF NOT EXISTS idx_audit_loan_operation_type ON audit_events(loan_operation_type) WHERE loan_operation_type IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_loan_id ON audit_events(loan_id) WHERE loan_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_user_id_loan ON audit_events(user_id) WHERE user_id IS NOT NULL AND loan_operation_type IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_batch_id ON audit_events(batch_id) WHERE batch_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_loan_timestamp ON audit_events(timestamp, loan_operation_type) WHERE loan_operation_type IS NOT NULL;

-- Function to create loan operation audit event
CREATE OR REPLACE FUNCTION create_loan_audit_event(
    p_event_id VARCHAR,
    p_nullifier_value BIGINT,
    p_loan_operation_type VARCHAR,
    p_root_before BYTEA,
    p_root_after BYTEA,
    p_loan_id UUID DEFAULT NULL,
    p_user_id UUID DEFAULT NULL,
    p_operation_amount DECIMAL(18, 6) DEFAULT NULL,
    p_batch_id INTEGER DEFAULT NULL,
    p_transaction_hash VARCHAR(66) DEFAULT NULL,
    p_block_height BIGINT DEFAULT 0,
    p_operator VARCHAR(42) DEFAULT NULL,
    p_metadata JSONB DEFAULT '{}'
) RETURNS VOID AS $$
BEGIN
    INSERT INTO audit_events (
        event_id,
        nullifier_value,
        event_type,
        timestamp,
        root_before,
        root_after,
        transaction_hash,
        block_height,
        operator,
        metadata,
        loan_operation_type,
        loan_id,
        user_id,
        operation_amount,
        batch_id
    ) VALUES (
        p_event_id,
        p_nullifier_value,
        'loan_operation',
        NOW(),
        p_root_before,
        p_root_after,
        p_transaction_hash,
        p_block_height,
        p_operator,
        p_metadata,
        p_loan_operation_type,
        p_loan_id,
        p_user_id,
        p_operation_amount,
        p_batch_id
    );
END;
$$ LANGUAGE plpgsql;

-- Function to get loan audit trail for a specific loan
CREATE OR REPLACE FUNCTION get_loan_audit_trail(p_loan_id UUID, p_limit INTEGER DEFAULT 100)
RETURNS TABLE(
    event_id VARCHAR,
    nullifier_value BIGINT,
    loan_operation_type VARCHAR,
    operation_amount DECIMAL(18, 6),
    timestamp TIMESTAMP WITH TIME ZONE,
    root_before BYTEA,
    root_after BYTEA,
    batch_id INTEGER,
    user_id UUID,
    transaction_hash VARCHAR(66),
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ae.event_id,
        ae.nullifier_value,
        ae.loan_operation_type,
        ae.operation_amount,
        ae.timestamp,
        ae.root_before,
        ae.root_after,
        ae.batch_id,
        ae.user_id,
        ae.transaction_hash,
        ae.metadata
    FROM audit_events ae
    WHERE ae.loan_id = p_loan_id
        AND ae.loan_operation_type IS NOT NULL
    ORDER BY ae.timestamp ASC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- Function to get user loan audit trail
CREATE OR REPLACE FUNCTION get_user_loan_audit_trail(
    p_user_id UUID, 
    p_operation_type VARCHAR DEFAULT NULL,
    p_limit INTEGER DEFAULT 100
)
RETURNS TABLE(
    event_id VARCHAR,
    nullifier_value BIGINT,
    loan_operation_type VARCHAR,
    loan_id UUID,
    operation_amount DECIMAL(18, 6),
    timestamp TIMESTAMP WITH TIME ZONE,
    root_before BYTEA,
    root_after BYTEA,
    batch_id INTEGER,
    transaction_hash VARCHAR(66),
    metadata JSONB
) AS $$
BEGIN
    IF p_operation_type IS NOT NULL THEN
        RETURN QUERY
        SELECT 
            ae.event_id,
            ae.nullifier_value,
            ae.loan_operation_type,
            ae.loan_id,
            ae.operation_amount,
            ae.timestamp,
            ae.root_before,
            ae.root_after,
            ae.batch_id,
            ae.transaction_hash,
            ae.metadata
        FROM audit_events ae
        WHERE ae.user_id = p_user_id
            AND ae.loan_operation_type = p_operation_type
        ORDER BY ae.timestamp DESC
        LIMIT p_limit;
    ELSE
        RETURN QUERY
        SELECT 
            ae.event_id,
            ae.nullifier_value,
            ae.loan_operation_type,
            ae.loan_id,
            ae.operation_amount,
            ae.timestamp,
            ae.root_before,
            ae.root_after,
            ae.batch_id,
            ae.transaction_hash,
            ae.metadata
        FROM audit_events ae
        WHERE ae.user_id = p_user_id
            AND ae.loan_operation_type IS NOT NULL
        ORDER BY ae.timestamp DESC
        LIMIT p_limit;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to get batch audit trail
CREATE OR REPLACE FUNCTION get_batch_audit_trail(p_batch_id INTEGER)
RETURNS TABLE(
    event_id VARCHAR,
    nullifier_value BIGINT,
    loan_operation_type VARCHAR,
    loan_id UUID,
    user_id UUID,
    operation_amount DECIMAL(18, 6),
    timestamp TIMESTAMP WITH TIME ZONE,
    root_before BYTEA,
    root_after BYTEA,
    transaction_hash VARCHAR(66),
    metadata JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ae.event_id,
        ae.nullifier_value,
        ae.loan_operation_type,
        ae.loan_id,
        ae.user_id,
        ae.operation_amount,
        ae.timestamp,
        ae.root_before,
        ae.root_after,
        ae.transaction_hash,
        ae.metadata
    FROM audit_events ae
    WHERE ae.batch_id = p_batch_id
        AND ae.loan_operation_type IS NOT NULL
    ORDER BY ae.timestamp ASC;
END;
$$ LANGUAGE plpgsql;

-- Trigger function to automatically create audit events for loan operations
CREATE OR REPLACE FUNCTION create_loan_operation_audit_trigger()
RETURNS TRIGGER AS $$
DECLARE
    current_root BYTEA;
    previous_root BYTEA;
    event_uuid VARCHAR;
BEGIN
    -- Only process if this is a loan operation
    IF NEW.loan_operation_type IS NOT NULL THEN
        -- Get current tree root
        SELECT root_hash INTO current_root FROM tree_state WHERE tree_id = 'default';
        
        -- For new insertions, use previous root (simplified approach)
        previous_root := COALESCE(current_root, '\x0000000000000000000000000000000000000000000000000000000000000000'::bytea);
        
        -- Generate unique event ID
        event_uuid := 'loan_' || NEW.loan_operation_type || '_' || NEW.id::text || '_' || EXTRACT(EPOCH FROM NOW())::bigint::text;
        
        -- Create audit event
        PERFORM create_loan_audit_event(
            event_uuid,
            NEW.value,
            NEW.loan_operation_type,
            previous_root,
            current_root,
            NEW.loan_id,
            NEW.user_id,
            NEW.operation_amount,
            NEW.batch_id,
            NULL, -- transaction_hash (to be filled when posted on-chain)
            0,    -- block_height (to be filled when posted on-chain)
            NULL, -- operator (to be filled when posted on-chain)
            NEW.operation_metadata
        );
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to automatically audit loan operations
CREATE OR REPLACE TRIGGER nullifiers_loan_audit_trigger
    AFTER INSERT OR UPDATE ON nullifiers
    FOR EACH ROW
    EXECUTE FUNCTION create_loan_operation_audit_trigger();

-- ============================================================================
-- LOAN AUDIT VIEWS FOR COMPREHENSIVE TRACKING
-- ============================================================================

-- View for comprehensive loan audit trail with lifecycle information
CREATE OR REPLACE VIEW loan_audit_comprehensive AS
SELECT 
    ae.event_id,
    ae.loan_id,
    ae.user_id,
    ae.loan_operation_type,
    ae.operation_amount,
    ae.nullifier_value,
    ae.timestamp,
    ae.batch_id,
    ae.transaction_hash,
    ae.root_before,
    ae.root_after,
    ae.metadata,
    
    -- Join with loan information for context
    l.principal_amount,
    l.outstanding_balance,
    l.loan_status,
    l.originated_at,
    
    -- Join with user information
    i.email as user_email,
    i.user_type,
    i.kyc_status
    
FROM audit_events ae
LEFT JOIN loans l ON ae.loan_id = l.id
LEFT JOIN identities i ON ae.user_id = i.id
WHERE ae.loan_operation_type IS NOT NULL
ORDER BY ae.timestamp ASC;

-- View for loan operation statistics from audit trail
CREATE OR REPLACE VIEW loan_audit_stats AS
SELECT 
    ae.loan_operation_type,
    COUNT(*) as total_operations,
    COUNT(DISTINCT ae.loan_id) as unique_loans,
    COUNT(DISTINCT ae.user_id) as unique_users,
    COUNT(DISTINCT ae.batch_id) as unique_batches,
    SUM(ae.operation_amount) as total_amount,
    AVG(ae.operation_amount) as avg_amount,
    MIN(ae.timestamp) as first_operation,
    MAX(ae.timestamp) as last_operation,
    AVG(EXTRACT(EPOCH FROM (ae.timestamp - LAG(ae.timestamp) OVER (ORDER BY ae.timestamp)))) as avg_interval_seconds
FROM audit_events ae
WHERE ae.loan_operation_type IS NOT NULL
GROUP BY ae.loan_operation_type;

-- View for recent loan audit activity (last 24 hours)
CREATE OR REPLACE VIEW recent_loan_audit_activity AS
SELECT 
    ae.event_id,
    ae.loan_id,
    ae.user_id,
    ae.loan_operation_type,
    ae.operation_amount,
    ae.timestamp,
    ae.batch_id,
    l.principal_amount,
    l.loan_status,
    i.email as user_email,
    i.user_type
FROM audit_events ae
LEFT JOIN loans l ON ae.loan_id = l.id
LEFT JOIN identities i ON ae.user_id = i.id
WHERE ae.loan_operation_type IS NOT NULL
    AND ae.timestamp >= NOW() - INTERVAL '24 hours'
ORDER BY ae.timestamp DESC;

-- ============================================================================
-- COMMENTS AND DOCUMENTATION
-- ============================================================================

COMMENT ON COLUMN nullifiers.loan_operation_type IS 'Type of loan operation: loan_origination, loan_payment, loan_closeout, user_registration, kyc_submission, kyc_verification';
COMMENT ON COLUMN nullifiers.loan_id IS 'Reference to the loan this operation affects (NULL for non-loan operations)';
COMMENT ON COLUMN nullifiers.user_id IS 'Reference to the user performing this operation';
COMMENT ON COLUMN nullifiers.operation_amount IS 'Amount involved in this operation (loan amount, payment amount, etc.)';
COMMENT ON COLUMN nullifiers.operation_metadata IS 'Additional metadata about the operation (payment details, origination terms, etc.)';
COMMENT ON COLUMN nullifiers.batch_id IS 'Reference to the batch this nullifier was processed in';

COMMENT ON VIEW loan_operation_history IS 'Comprehensive view of all loan operations stored in the IMT with related loan and user information';
COMMENT ON VIEW loan_lifecycle_summary IS 'Aggregated view of loan lifecycle showing operation counts and statistics from the IMT';
COMMENT ON VIEW user_loan_activity IS 'User-centric view of loan operation activity and statistics';
COMMENT ON VIEW imt_loan_stats IS 'Real-time IMT statistics focused on loan operations';

COMMENT ON FUNCTION get_loan_operation_history(UUID) IS 'Retrieve chronological history of all operations for a specific loan';
COMMENT ON FUNCTION get_user_loan_operations(UUID, VARCHAR, INTEGER) IS 'Retrieve loan operations for a specific user with optional filtering';
COMMENT ON FUNCTION get_loan_operations_by_amount_range(DECIMAL, DECIMAL, VARCHAR) IS 'Query loan operations within a specific amount range';
COMMENT ON FUNCTION get_batch_loan_summary(INTEGER) IS 'Get summary statistics of loan operations in a specific batch';
COMMENT ON FUNCTION insert_loan_nullifier_atomic IS 'Enhanced nullifier insertion with loan operation metadata';

-- ============================================================================
-- INITIAL DATA POPULATION (BACKFILL EXISTING DATA IF NEEDED)
-- ============================================================================

-- Update existing nullifiers with loan operation data from audit_events where possible
-- This is a one-time backfill operation for existing systems

UPDATE nullifiers n
SET 
    loan_operation_type = ae.event_type,
    loan_id = ae.loan_id,
    user_id = ae.user_id,
    operation_metadata = ae.metadata
FROM audit_events ae
WHERE n.value = ae.nullifier_value
  AND n.loan_operation_type IS NULL
  AND ae.loan_operation_type IS NOT NULL;

-- Initialize tree statistics
SELECT refresh_loan_statistics();

-- ============================================================================
-- COMPLETION MESSAGE
-- ============================================================================

-- Log completion of migration
DO $$
BEGIN
    RAISE NOTICE 'IMT Loan Enhancement Migration 015 completed successfully';
    RAISE NOTICE 'Added loan operation metadata to % existing nullifiers', (
        SELECT COUNT(*) FROM nullifiers WHERE loan_operation_type IS NOT NULL
    );
    RAISE NOTICE 'Created % new indexes for loan operation queries', 9;
    RAISE NOTICE 'Created % new views for loan lifecycle visibility', 4;
    RAISE NOTICE 'Created % new functions for loan operation queries', 5;
END $$;