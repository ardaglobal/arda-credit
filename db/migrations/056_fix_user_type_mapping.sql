-- Fix user type mapping in sync function
-- This migration updates the sync trigger to properly handle the "user" user_type
-- that exists in the identities table but isn't allowed in the users table constraint.

-- Update the sync function to map "user" type to "investor"
CREATE OR REPLACE FUNCTION sync_users_with_identities()
RETURNS TRIGGER AS $$
BEGIN
    -- Handle INSERT
    IF TG_OP = 'INSERT' THEN
        INSERT INTO users (id, email, user_type, kyc_status, created_at, updated_at)
        VALUES (
            NEW.id,
            NEW.email,
            CASE
                WHEN NEW.user_type IN ('investor', 'lender', 'merchant', 'admin') THEN NEW.user_type
                WHEN NEW.user_type = 'user' THEN 'investor'  -- Map "user" to "investor"
                ELSE 'investor'
            END,
            NEW.kyc_status,
            NEW.created_at,
            NEW.updated_at
        )
        ON CONFLICT (id) DO UPDATE SET
            email = EXCLUDED.email,
            user_type = EXCLUDED.user_type,
            kyc_status = EXCLUDED.kyc_status,
            updated_at = EXCLUDED.updated_at;
        RETURN NEW;
    END IF;

    -- Handle UPDATE
    IF TG_OP = 'UPDATE' THEN
        UPDATE users SET
            email = NEW.email,
            user_type = CASE
                WHEN NEW.user_type IN ('investor', 'lender', 'merchant', 'admin') THEN NEW.user_type
                WHEN NEW.user_type = 'user' THEN 'investor'  -- Map "user" to "investor"
                ELSE COALESCE(OLD.user_type, 'investor')
            END,
            kyc_status = NEW.kyc_status,
            updated_at = NEW.updated_at
        WHERE id = NEW.id;
        RETURN NEW;
    END IF;

    -- Handle DELETE
    IF TG_OP = 'DELETE' THEN
        DELETE FROM users WHERE id = OLD.id;
        RETURN OLD;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Update existing users table records that might have invalid user_type values
UPDATE users
SET user_type = 'investor'
WHERE user_type NOT IN ('investor', 'lender', 'merchant', 'admin');