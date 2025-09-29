-- Fix column constraints and types to match Rust struct expectations

-- Make investor_id NOT NULL in liquidity_pools since Rust structs expect Uuid, not Option<Uuid>
-- First set a default value for existing NULL records
UPDATE liquidity_pools SET investor_id = '00000000-0000-0000-0000-000000000000' WHERE investor_id IS NULL;
ALTER TABLE liquidity_pools ALTER COLUMN investor_id SET NOT NULL;

-- Make loan_status NOT NULL in deals table since it should always have a status
UPDATE deals SET loan_status = status WHERE loan_status IS NULL;
ALTER TABLE deals ALTER COLUMN loan_status SET NOT NULL;

-- Make listing_id NOT NULL in deal_listings since Rust expects Uuid, not Option<Uuid>
UPDATE deal_listings SET listing_id = id WHERE listing_id IS NULL;
ALTER TABLE deal_listings ALTER COLUMN listing_id SET NOT NULL;

-- Make available_balance NOT NULL in user_balances
UPDATE user_balances SET available_balance = 0 WHERE available_balance IS NULL;
ALTER TABLE user_balances ALTER COLUMN available_balance SET NOT NULL;

-- Fix investor profiles boolean fields to be NOT NULL
ALTER TABLE investor_profiles ADD COLUMN IF NOT EXISTS accredited_investor BOOLEAN DEFAULT FALSE;
ALTER TABLE investor_profiles ADD COLUMN IF NOT EXISTS verified BOOLEAN DEFAULT FALSE;
ALTER TABLE investor_profiles ADD COLUMN IF NOT EXISTS active BOOLEAN DEFAULT TRUE;

UPDATE investor_profiles SET accredited_investor = FALSE WHERE accredited_investor IS NULL;
UPDATE investor_profiles SET verified = FALSE WHERE verified IS NULL;
UPDATE investor_profiles SET active = TRUE WHERE active IS NULL;

ALTER TABLE investor_profiles ALTER COLUMN accredited_investor SET NOT NULL;
ALTER TABLE investor_profiles ALTER COLUMN verified SET NOT NULL;
ALTER TABLE investor_profiles ALTER COLUMN active SET NOT NULL;