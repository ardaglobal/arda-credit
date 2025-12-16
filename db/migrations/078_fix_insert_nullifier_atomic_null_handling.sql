-- Fix insert_nullifier_atomic to handle NULL values properly
-- Migration 078: Fix null handling in insert_nullifier_atomic

CREATE OR REPLACE FUNCTION insert_nullifier_atomic(new_value BIGINT)
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
    -- Step 1: Find low nullifier (works correctly with genesis nullifier)
    SELECT * INTO low_null
    FROM find_low_nullifier(new_value)
    LIMIT 1;
    
    -- Step 2: Validate insertion is possible
    IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
        -- Nullifier already exists
        RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
        RETURN;
    END IF;
    
    -- Step 3: Get tree index atomically
    actual_tree_index := get_next_tree_index();
    
    BEGIN
        -- Step 4: Insert new nullifier with COALESCE to handle NULL values
        INSERT INTO nullifiers (value, next_index, next_value, tree_index)
        VALUES (
            new_value,
            low_null.low_next_index,
            COALESCE(low_null.low_next_value, 0),  -- Ensure NOT NULL
            actual_tree_index
        );
        
        -- Step 5: Update the low nullifier to point to new nullifier (skip if virtual/NULL)
        IF low_null.low_value IS NOT NULL AND low_null.low_value != 0 THEN
            UPDATE nullifiers
            SET
                next_index = actual_tree_index,
                next_value = new_value
            WHERE value = low_null.low_value AND is_active = true;
        END IF;
        
        -- Step 6: Update tree state
        UPDATE tree_state
        SET 
            total_nullifiers = total_nullifiers + 1,
            updated_at = NOW()
        WHERE tree_id = 'default';
        
        -- Step 7: Return success result
        RETURN QUERY SELECT
            actual_tree_index,
            COALESCE(low_null.low_value, 0),
            COALESCE(low_null.low_next_value, 0),
            TRUE;
            
    EXCEPTION WHEN OTHERS THEN
        -- Rollback on any error
        RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
    END;
END;
$$ LANGUAGE plpgsql;

