-- Add missing schema for liquidity pools and nullifiers

-- Add investor_id to liquidity_pools table
ALTER TABLE liquidity_pools ADD COLUMN investor_id UUID;

-- Add foreign key constraint for investor_id
ALTER TABLE liquidity_pools ADD CONSTRAINT fk_liquidity_pools_investor_id
    FOREIGN KEY (investor_id) REFERENCES users(id);

-- Create nullifiers table for Merkle tree operations
CREATE TABLE nullifiers (
    id SERIAL PRIMARY KEY,
    nullifier_value BYTEA NOT NULL UNIQUE,
    leaf_index BIGINT NOT NULL,
    is_used BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create index for performance
CREATE INDEX idx_nullifiers_value ON nullifiers(nullifier_value);
CREATE INDEX idx_nullifiers_leaf_index ON nullifiers(leaf_index);
CREATE INDEX idx_nullifiers_is_used ON nullifiers(is_used);

-- Create find_low_nullifier function (placeholder implementation)
CREATE OR REPLACE FUNCTION find_low_nullifier(search_value BYTEA)
RETURNS TABLE (
    nullifier_value BYTEA,
    leaf_index BIGINT,
    next_nullifier_value BYTEA,
    next_leaf_index BIGINT
) AS $$
BEGIN
    -- Placeholder implementation - find the highest nullifier less than search_value
    RETURN QUERY
    SELECT
        n.nullifier_value,
        n.leaf_index,
        next_n.nullifier_value as next_nullifier_value,
        next_n.leaf_index as next_leaf_index
    FROM nullifiers n
    LEFT JOIN nullifiers next_n ON next_n.nullifier_value > n.nullifier_value
    WHERE n.nullifier_value <= search_value
    ORDER BY n.nullifier_value DESC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql;

-- Create insert_nullifier_atomic function (placeholder implementation)
CREATE OR REPLACE FUNCTION insert_nullifier_atomic(new_nullifier BYTEA)
RETURNS BIGINT AS $$
DECLARE
    new_leaf_index BIGINT;
BEGIN
    -- Insert nullifier and return leaf index
    INSERT INTO nullifiers (nullifier_value, leaf_index, is_used)
    VALUES (new_nullifier, COALESCE((SELECT MAX(leaf_index) FROM nullifiers), 0) + 1, TRUE)
    RETURNING leaf_index INTO new_leaf_index;

    RETURN new_leaf_index;
END;
$$ LANGUAGE plpgsql;

-- Add comments for documentation
COMMENT ON TABLE nullifiers IS 'Merkle tree nullifier tracking for ZK proofs';
COMMENT ON FUNCTION find_low_nullifier(BYTEA) IS 'Find the largest nullifier less than or equal to search value';
COMMENT ON FUNCTION insert_nullifier_atomic(BYTEA) IS 'Atomically insert a nullifier and return its leaf index';