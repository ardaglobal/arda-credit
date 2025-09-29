-- ============================================================================
-- REFACTOR STATE TRANSITIONS SYSTEM
-- Enhance transaction_queue → state_transitions with comprehensive audit trail
-- ============================================================================

-- ============================================================================
-- ENHANCED STATE TRANSITION TYPES
-- ============================================================================

-- Add new transition types for comprehensive coverage
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'company_registration';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'company_kyb_submission';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'company_kyb_verification';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'user_update';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'deal_update';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'deal_status_change';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'marketplace_listing';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'marketplace_delisting';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'ownership_transfer';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'pool_status_update';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'credit_line_request';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'credit_line_approval';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'withdrawal_request';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'withdrawal_approval';
ALTER TYPE transaction_type ADD VALUE IF NOT EXISTS 'withdrawal_rejection';

-- ============================================================================
-- RENAME AND ENHANCE TABLE STRUCTURE
-- ============================================================================

-- First, rename the table to better reflect its purpose
ALTER TABLE transaction_queue RENAME TO state_transitions;

-- Add new columns for comprehensive state tracking
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS transition_id UUID DEFAULT gen_random_uuid();
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS before_state JSONB; -- State before transition
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS after_state JSONB;  -- State after transition
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS proof_hash BYTEA;   -- ZK proof hash
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS state_root_before BYTEA; -- Merkle root before
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS state_root_after BYTEA;  -- Merkle root after
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS validated_at TIMESTAMP WITH TIME ZONE; -- When state was validated
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS validator_node VARCHAR(100); -- Which node validated
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS block_number BIGINT; -- Block number if on-chain
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS gas_used INTEGER; -- Gas consumption tracking
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS error_message TEXT; -- Error details if transition failed
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS retry_count INTEGER DEFAULT 0; -- Number of retry attempts
ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS parent_transition_id UUID REFERENCES state_transitions(id); -- For linked transitions

-- Add proper constraints
ALTER TABLE state_transitions ADD CONSTRAINT unique_transition_id UNIQUE (transition_id);
ALTER TABLE state_transitions ADD CONSTRAINT valid_retry_count CHECK (retry_count >= 0);
ALTER TABLE state_transitions ADD CONSTRAINT valid_gas_used CHECK (gas_used >= 0 OR gas_used IS NULL);

-- Update existing column constraints
ALTER TABLE state_transitions ALTER COLUMN transition_id SET NOT NULL;

-- ============================================================================
-- STATE VALIDATION ENUM
-- ============================================================================

CREATE TYPE state_validation_status AS ENUM (
    'pending',      -- Waiting for validation
    'validating',   -- Currently being validated
    'valid',        -- State matches ZK circuit
    'invalid',      -- State mismatch detected
    'error',        -- Validation error occurred
    'retrying'      -- Retrying validation
);

ALTER TABLE state_transitions ADD COLUMN IF NOT EXISTS validation_status state_validation_status DEFAULT 'pending';

-- ============================================================================
-- ENHANCED INDEXES FOR PERFORMANCE
-- ============================================================================

-- Drop old indexes from transaction_queue
DROP INDEX IF EXISTS idx_transaction_queue_user_id;
DROP INDEX IF EXISTS idx_transaction_queue_deal_id;
DROP INDEX IF EXISTS idx_transaction_queue_transaction_type;
DROP INDEX IF EXISTS idx_transaction_queue_created_at;
DROP INDEX IF EXISTS idx_transaction_queue_batch_id;
DROP INDEX IF EXISTS idx_transaction_queue_status;

-- Create comprehensive indexes for state_transitions
CREATE INDEX idx_state_transitions_transition_id ON state_transitions(transition_id);
CREATE INDEX idx_state_transitions_user_id ON state_transitions(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX idx_state_transitions_deal_id ON state_transitions(deal_id) WHERE deal_id IS NOT NULL;
CREATE INDEX idx_state_transitions_company_id ON state_transitions(company_id) WHERE company_id IS NOT NULL;
CREATE INDEX idx_state_transitions_payment_id ON state_transitions(payment_id) WHERE payment_id IS NOT NULL;
CREATE INDEX idx_state_transitions_transaction_type ON state_transitions(transaction_type);
CREATE INDEX idx_state_transitions_created_at ON state_transitions(created_at);
CREATE INDEX idx_state_transitions_validated_at ON state_transitions(validated_at) WHERE validated_at IS NOT NULL;
CREATE INDEX idx_state_transitions_validation_status ON state_transitions(validation_status);
CREATE INDEX idx_state_transitions_batch_id ON state_transitions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_state_transitions_unbatched ON state_transitions(created_at) WHERE included_in_batch_id IS NULL;
CREATE INDEX idx_state_transitions_proof_hash ON state_transitions(proof_hash) WHERE proof_hash IS NOT NULL;
CREATE INDEX idx_state_transitions_parent_id ON state_transitions(parent_transition_id) WHERE parent_transition_id IS NOT NULL;

-- Composite indexes for common query patterns
CREATE INDEX idx_state_transitions_user_type_date ON state_transitions(user_id, transaction_type, created_at) WHERE user_id IS NOT NULL;
CREATE INDEX idx_state_transitions_deal_timeline ON state_transitions(deal_id, created_at) WHERE deal_id IS NOT NULL;
CREATE INDEX idx_state_transitions_validation_pending ON state_transitions(validation_status, created_at) WHERE validation_status = 'pending';
CREATE INDEX idx_state_transitions_batch_processing ON state_transitions(included_in_batch_id, batch_position) WHERE included_in_batch_id IS NOT NULL;

-- GIN indexes for JSONB state data
CREATE INDEX idx_state_transitions_before_state_gin ON state_transitions USING GIN(before_state) WHERE before_state IS NOT NULL;
CREATE INDEX idx_state_transitions_after_state_gin ON state_transitions USING GIN(after_state) WHERE after_state IS NOT NULL;
CREATE INDEX idx_state_transitions_metadata_gin ON state_transitions USING GIN(metadata) WHERE metadata IS NOT NULL;

-- ============================================================================
-- STATE TRANSITION HELPER FUNCTIONS
-- ============================================================================

-- Function to create a state transition record
CREATE OR REPLACE FUNCTION create_state_transition(
    p_transaction_type transaction_type,
    p_user_id UUID DEFAULT NULL,
    p_deal_id UUID DEFAULT NULL,
    p_company_id UUID DEFAULT NULL,
    p_payment_id UUID DEFAULT NULL,
    p_amount DECIMAL(18, 6) DEFAULT 0,
    p_before_state JSONB DEFAULT NULL,
    p_after_state JSONB DEFAULT NULL,
    p_metadata JSONB DEFAULT NULL,
    p_parent_transition_id UUID DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    transition_record_id UUID;
    user_hash BYTEA;
    deal_hash BYTEA;
    transaction_id_hash BYTEA;
BEGIN
    -- Generate UUID for this transition
    SELECT gen_random_uuid() INTO transition_record_id;

    -- Generate required hashes for ZK compatibility
    IF p_user_id IS NOT NULL THEN
        user_hash := digest(p_user_id::text, 'sha256');
    END IF;

    IF p_deal_id IS NOT NULL THEN
        deal_hash := digest(p_deal_id::text, 'sha256');
    END IF;

    transaction_id_hash := digest(transition_record_id::text, 'sha256');

    -- Insert the state transition record
    INSERT INTO state_transitions (
        id,
        transition_id,
        transaction_id_hash,
        user_id_hash,
        deal_id_hash,
        transaction_type,
        amount,
        user_id,
        deal_id,
        company_id,
        payment_id,
        before_state,
        after_state,
        metadata,
        parent_transition_id
    ) VALUES (
        transition_record_id,
        transition_record_id,
        transaction_id_hash,
        user_hash,
        deal_hash,
        p_transaction_type,
        p_amount,
        p_user_id,
        p_deal_id,
        p_company_id,
        p_payment_id,
        p_before_state,
        p_after_state,
        p_metadata,
        p_parent_transition_id
    );

    RETURN transition_record_id;
END;
$$ LANGUAGE plpgsql;

-- Function to validate state transitions match ZK circuit expectations
CREATE OR REPLACE FUNCTION validate_state_transition(
    p_transition_id UUID,
    p_expected_state_root BYTEA DEFAULT NULL
) RETURNS BOOLEAN AS $$
DECLARE
    transition_record state_transitions%ROWTYPE;
    validation_result BOOLEAN := TRUE;
    error_details TEXT := '';
BEGIN
    -- Get the transition record
    SELECT * INTO transition_record
    FROM state_transitions
    WHERE transition_id = p_transition_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'State transition not found: %', p_transition_id;
    END IF;

    -- Basic validation checks
    IF transition_record.before_state IS NOT NULL AND transition_record.after_state IS NOT NULL THEN
        -- Validate that state actually changed (for state-changing operations)
        IF transition_record.before_state = transition_record.after_state THEN
            CASE transition_record.transaction_type
                WHEN 'deal_origination', 'deal_payment', 'deal_closeout', 'kyc_verification',
                     'user_registration', 'company_registration', 'deal_transfer',
                     'marketplace_listing', 'pool_creation' THEN
                    validation_result := FALSE;
                    error_details := 'State unchanged for state-changing operation';
                ELSE
                    -- Some operations may not change state (queries, validations)
                    NULL;
            END CASE;
        END IF;
    END IF;

    -- Validate state root progression if provided
    IF p_expected_state_root IS NOT NULL AND transition_record.state_root_after IS NOT NULL THEN
        IF transition_record.state_root_after != p_expected_state_root THEN
            validation_result := FALSE;
            error_details := error_details || '; State root mismatch';
        END IF;
    END IF;

    -- Update validation status
    UPDATE state_transitions
    SET
        validation_status = CASE
            WHEN validation_result THEN 'valid'::state_validation_status
            ELSE 'invalid'::state_validation_status
        END,
        validated_at = NOW(),
        error_message = CASE
            WHEN validation_result THEN NULL
            ELSE error_details
        END
    WHERE transition_id = p_transition_id;

    RETURN validation_result;
END;
$$ LANGUAGE plpgsql;

-- Function to get state transition history for an entity
CREATE OR REPLACE FUNCTION get_state_history(
    p_entity_type VARCHAR,  -- 'user', 'deal', 'company'
    p_entity_id UUID,
    p_limit INTEGER DEFAULT 100
) RETURNS TABLE(
    transition_id UUID,
    transaction_type transaction_type,
    before_state JSONB,
    after_state JSONB,
    created_at TIMESTAMP WITH TIME ZONE,
    validation_status state_validation_status,
    proof_hash BYTEA
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.transition_id,
        st.transaction_type,
        st.before_state,
        st.after_state,
        st.created_at,
        st.validation_status,
        st.proof_hash
    FROM state_transitions st
    WHERE
        CASE p_entity_type
            WHEN 'user' THEN st.user_id = p_entity_id
            WHEN 'deal' THEN st.deal_id = p_entity_id
            WHEN 'company' THEN st.company_id = p_entity_id
            ELSE FALSE
        END
    ORDER BY st.created_at DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- Function to check database state alignment with ZK circuit
CREATE OR REPLACE FUNCTION check_state_alignment()
RETURNS TABLE(
    check_name TEXT,
    status TEXT,
    details TEXT
) AS $$
BEGIN
    -- Check for unvalidated transitions
    RETURN QUERY
    SELECT
        'unvalidated_transitions'::TEXT,
        CASE
            WHEN COUNT(*) = 0 THEN 'PASS'
            ELSE 'WARN'
        END::TEXT,
        'Found ' || COUNT(*)::TEXT || ' unvalidated state transitions'::TEXT
    FROM state_transitions
    WHERE validation_status = 'pending'
    AND created_at < NOW() - INTERVAL '1 hour';

    -- Check for failed validations
    RETURN QUERY
    SELECT
        'failed_validations'::TEXT,
        CASE
            WHEN COUNT(*) = 0 THEN 'PASS'
            ELSE 'FAIL'
        END::TEXT,
        'Found ' || COUNT(*)::TEXT || ' failed state validations'::TEXT
    FROM state_transitions
    WHERE validation_status = 'invalid';

    -- Check for orphaned transitions (no batch assignment after reasonable time)
    RETURN QUERY
    SELECT
        'orphaned_transitions'::TEXT,
        CASE
            WHEN COUNT(*) = 0 THEN 'PASS'
            ELSE 'WARN'
        END::TEXT,
        'Found ' || COUNT(*)::TEXT || ' transitions not assigned to batches'::TEXT
    FROM state_transitions
    WHERE included_in_batch_id IS NULL
    AND created_at < NOW() - INTERVAL '6 hours'
    AND transaction_type IN ('deal_origination', 'deal_payment', 'deal_closeout', 'kyc_verification');

    RETURN;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- UPDATE BATCH PROCESSING FUNCTIONS
-- ============================================================================

-- Update the existing batch creation function to use new table name
CREATE OR REPLACE FUNCTION get_unbatched_transitions(limit_count INTEGER DEFAULT 10)
RETURNS TABLE(
    transaction_id UUID,
    transition_id UUID,
    transaction_id_hash BYTEA,
    user_id_hash BYTEA,
    deal_id_hash BYTEA,
    transaction_type transaction_type,
    amount DECIMAL(18, 6),
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        st.id,
        st.transition_id,
        st.transaction_id_hash,
        st.user_id_hash,
        st.deal_id_hash,
        st.transaction_type,
        st.amount,
        st.created_at
    FROM state_transitions st
    WHERE st.included_in_batch_id IS NULL
    ORDER BY st.created_at ASC
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Update references in batch creation function
CREATE OR REPLACE FUNCTION create_batch_with_transitions(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    arithmetic_total INTEGER;
    arithmetic_transaction_ids INTEGER[];
    zk_transition_ids UUID[];
BEGIN
    -- Get current counter value
    SELECT COALESCE(counter_value, 0) INTO previous_counter
    FROM global_state
    LIMIT 1;

    -- If no global_state record exists, create one
    IF previous_counter IS NULL THEN
        INSERT INTO global_state (counter_value) VALUES (0);
        previous_counter := 0;
    END IF;

    -- Get unbatched arithmetic transactions (legacy support)
    SELECT
        ARRAY_AGG(id ORDER BY id),
        SUM(amount)
    INTO arithmetic_transaction_ids, arithmetic_total
    FROM (
        SELECT id, amount
        FROM incoming_transactions
        WHERE included_in_batch_id IS NULL
        ORDER BY id ASC
        LIMIT batch_size / 2
    ) unbatched;

    -- Get unbatched state transitions
    SELECT ARRAY_AGG(id ORDER BY created_at)
    INTO zk_transition_ids
    FROM (
        SELECT id
        FROM state_transitions
        WHERE included_in_batch_id IS NULL
        ORDER BY created_at ASC
        LIMIT batch_size / 2
    ) unbatched_transitions;

    -- Return 0 if no transactions to batch
    IF (arithmetic_transaction_ids IS NULL OR array_length(arithmetic_transaction_ids, 1) = 0) AND
       (zk_transition_ids IS NULL OR array_length(zk_transition_ids, 1) = 0) THEN
        RETURN 0;
    END IF;

    -- Calculate final counter value
    final_counter := previous_counter + COALESCE(arithmetic_total, 0);

    -- Create new batch
    INSERT INTO proof_batches (
        previous_counter_value,
        final_counter_value,
        transaction_ids
    ) VALUES (
        previous_counter,
        final_counter,
        COALESCE(arithmetic_transaction_ids, ARRAY[]::INTEGER[])
    ) RETURNING id INTO new_batch_id;

    -- Mark arithmetic transactions as included
    IF arithmetic_transaction_ids IS NOT NULL AND array_length(arithmetic_transaction_ids, 1) > 0 THEN
        UPDATE incoming_transactions
        SET included_in_batch_id = new_batch_id
        WHERE id = ANY(arithmetic_transaction_ids);
    END IF;

    -- Mark state transitions as included
    IF zk_transition_ids IS NOT NULL AND array_length(zk_transition_ids, 1) > 0 THEN
        UPDATE state_transitions
        SET
            included_in_batch_id = new_batch_id,
            validation_status = 'validating'
        WHERE id = ANY(zk_transition_ids);
    END IF;

    -- Update global state
    UPDATE global_state
    SET
        counter_value = final_counter,
        last_batch_id = new_batch_id,
        last_updated = NOW();

    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC STATE TRACKING
-- ============================================================================

-- Function to automatically create state transitions for key operations
CREATE OR REPLACE FUNCTION auto_create_state_transition()
RETURNS TRIGGER AS $$
DECLARE
    transition_type transaction_type;
    before_state JSONB;
    after_state JSONB;
    entity_id UUID;
BEGIN
    -- Determine transition type and entity based on table
    CASE TG_TABLE_NAME
        WHEN 'users' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'user_registration';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            ELSIF TG_OP = 'UPDATE' THEN
                transition_type := 'user_update';
                entity_id := NEW.id;
                before_state := to_jsonb(OLD);
                after_state := to_jsonb(NEW);
                -- Special handling for KYC status changes
                IF OLD.kyc_status != NEW.kyc_status THEN
                    transition_type := 'kyc_verification';
                END IF;
            END IF;

        WHEN 'companies' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'company_registration';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            ELSIF TG_OP = 'UPDATE' THEN
                transition_type := 'company_kyb_verification';
                entity_id := NEW.id;
                before_state := to_jsonb(OLD);
                after_state := to_jsonb(NEW);
            END IF;

        WHEN 'deals' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'deal_origination';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            ELSIF TG_OP = 'UPDATE' THEN
                IF OLD.status != NEW.status THEN
                    transition_type := 'deal_status_change';
                ELSE
                    transition_type := 'deal_update';
                END IF;
                entity_id := NEW.id;
                before_state := to_jsonb(OLD);
                after_state := to_jsonb(NEW);
            END IF;

        WHEN 'deal_payments' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'deal_payment';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            END IF;

        WHEN 'deal_listings' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'marketplace_listing';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            ELSIF TG_OP = 'DELETE' THEN
                transition_type := 'marketplace_delisting';
                entity_id := OLD.id;
                before_state := to_jsonb(OLD);
                after_state := NULL;
            END IF;

        WHEN 'deal_transfers' THEN
            IF TG_OP = 'INSERT' THEN
                transition_type := 'ownership_transfer';
                entity_id := NEW.id;
                before_state := NULL;
                after_state := to_jsonb(NEW);
            END IF;

        ELSE
            -- Unknown table, skip
            RETURN COALESCE(NEW, OLD);
    END CASE;

    -- Create the state transition record
    PERFORM create_state_transition(
        p_transaction_type := transition_type,
        p_user_id := CASE
            WHEN TG_TABLE_NAME = 'users' THEN entity_id
            WHEN TG_TABLE_NAME = 'deal_payments' THEN (SELECT payer_id FROM deal_payments WHERE id = entity_id)
            ELSE NULL
        END,
        p_deal_id := CASE
            WHEN TG_TABLE_NAME = 'deals' THEN entity_id
            WHEN TG_TABLE_NAME = 'deal_payments' THEN (SELECT deal_id FROM deal_payments WHERE id = entity_id)
            WHEN TG_TABLE_NAME = 'deal_listings' THEN (SELECT deal_id FROM deal_listings WHERE id = entity_id)
            WHEN TG_TABLE_NAME = 'deal_transfers' THEN (SELECT deal_id FROM deal_transfers WHERE id = entity_id)
            ELSE NULL
        END,
        p_company_id := CASE
            WHEN TG_TABLE_NAME = 'companies' THEN entity_id
            WHEN TG_TABLE_NAME = 'users' THEN (SELECT company_id FROM users WHERE id = entity_id)
            ELSE NULL
        END,
        p_payment_id := CASE
            WHEN TG_TABLE_NAME = 'deal_payments' THEN entity_id
            ELSE NULL
        END,
        p_before_state := before_state,
        p_after_state := after_state
    );

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Create triggers for automatic state transition tracking
CREATE TRIGGER trigger_users_state_transition
    AFTER INSERT OR UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

CREATE TRIGGER trigger_companies_state_transition
    AFTER INSERT OR UPDATE ON companies
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

CREATE TRIGGER trigger_deals_state_transition
    AFTER INSERT OR UPDATE ON deals
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

CREATE TRIGGER trigger_deal_payments_state_transition
    AFTER INSERT ON deal_payments
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

CREATE TRIGGER trigger_deal_listings_state_transition
    AFTER INSERT OR DELETE ON deal_listings
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

CREATE TRIGGER trigger_deal_transfers_state_transition
    AFTER INSERT ON deal_transfers
    FOR EACH ROW EXECUTE FUNCTION auto_create_state_transition();

-- ============================================================================
-- UPDATE EXISTING FUNCTION REFERENCES
-- ============================================================================

-- Drop the old function that referenced transaction_queue
DROP FUNCTION IF EXISTS get_unbatched_transactions(INTEGER);

-- Update comment references
COMMENT ON TABLE state_transitions IS 'Comprehensive state transition audit trail for ZK proof integration with before/after state tracking';
COMMENT ON TABLE proof_batches IS 'Batches of state transitions with ZK proofs for privacy-preserving verification';

COMMENT ON COLUMN state_transitions.transition_id IS 'Unique identifier for this state transition';
COMMENT ON COLUMN state_transitions.before_state IS 'JSONB representation of entity state before transition';
COMMENT ON COLUMN state_transitions.after_state IS 'JSONB representation of entity state after transition';
COMMENT ON COLUMN state_transitions.proof_hash IS 'Hash of the ZK proof for this transition';
COMMENT ON COLUMN state_transitions.state_root_before IS 'Merkle tree root before this transition';
COMMENT ON COLUMN state_transitions.state_root_after IS 'Merkle tree root after this transition';
COMMENT ON COLUMN state_transitions.validation_status IS 'Status of database-ZK circuit state alignment validation';
COMMENT ON COLUMN state_transitions.validated_at IS 'Timestamp when state validation was completed';
COMMENT ON COLUMN state_transitions.parent_transition_id IS 'Reference to parent transition for linked operations';