-- Fix find_low_nullifier to properly handle insertions smaller than all existing nullifiers
-- Migration 076: Fix find_low_nullifier for small values

-- Drop and recreate the find_low_nullifier function with proper handling for all cases
CREATE OR REPLACE FUNCTION find_low_nullifier(new_value BIGINT)
RETURNS TABLE(
    low_value BIGINT,
    low_next_index BIGINT,
    low_next_value BIGINT,
    low_tree_index BIGINT
) AS $$
BEGIN
    -- Case 1: Find nullifier where: low_nullifier.value < new_value
    -- AND (low_nullifier.next_value > new_value OR low_nullifier.next_value = 0)
    -- This is the normal case where new_value fits in the gap (low_value, next_value)
    RETURN QUERY
    SELECT 
        n.value,
        n.next_index,
        n.next_value,
        n.tree_index
    FROM nullifiers n
    WHERE n.is_active = true
      AND n.value < new_value
      AND (n.next_value > new_value OR n.next_value = 0)
    ORDER BY n.value DESC
    LIMIT 1;
    
    -- Case 2: If no result found, new_value must be smaller than all existing nullifiers
    -- Return a virtual low nullifier (value=0) that points to the smallest existing nullifier
    IF NOT FOUND THEN
        RETURN QUERY
        SELECT 
            0::BIGINT as value,
            n.tree_index as next_index,
            n.value as next_value,
            0::BIGINT as tree_index
        FROM nullifiers n
        WHERE n.is_active = true
        ORDER BY n.value ASC
        LIMIT 1;
    END IF;
END;
$$ LANGUAGE plpgsql;

