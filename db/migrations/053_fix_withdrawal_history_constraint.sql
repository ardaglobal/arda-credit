-- Fix withdrawal request history constraint to include 'approval' and 'state_change' event types
-- This addresses the constraint violation when approving withdrawals

-- Drop the existing constraint
ALTER TABLE withdrawal_request_history DROP CONSTRAINT valid_event_type;

-- Add the updated constraint that includes 'approval' and 'state_change'
ALTER TABLE withdrawal_request_history ADD CONSTRAINT valid_event_type
CHECK (event_type IN ('kyc_validation', 'balance_validation', 'contract_submission', 'confirmation_check', 'error', 'cancellation', 'retry', 'approval', 'state_change'));

-- Update the trigger function to handle 'approved' state properly
CREATE OR REPLACE FUNCTION create_withdrawal_request_history()
RETURNS TRIGGER AS $$
BEGIN
    -- Only create history entry if state actually changed
    IF OLD.state IS DISTINCT FROM NEW.state THEN
        INSERT INTO withdrawal_request_history (
            withdrawal_request_id,
            event_type,
            old_state,
            new_state,
            success,
            message,
            details
        ) VALUES (
            NEW.id,
            CASE
                WHEN NEW.state = 'approved' THEN 'approval'
                WHEN NEW.state = 'validated' THEN 'balance_validation'
                WHEN NEW.state = 'submitted' THEN 'contract_submission'
                WHEN NEW.state = 'confirmed' THEN 'confirmation_check'
                WHEN NEW.state = 'failed' THEN 'error'
                WHEN NEW.state = 'cancelled' THEN 'cancellation'
                ELSE 'state_change'
            END,
            OLD.state,
            NEW.state,
            NEW.state NOT IN ('failed', 'cancelled'),
            CASE
                WHEN NEW.state = 'approved' THEN 'Withdrawal approved by admin'
                WHEN NEW.state = 'validated' THEN 'KYC and balance validation completed'
                WHEN NEW.state = 'submitted' THEN 'Withdrawal submitted to smart contract'
                WHEN NEW.state = 'confirmed' THEN 'Withdrawal confirmed on blockchain'
                WHEN NEW.state = 'failed' THEN COALESCE(NEW.error_message, 'Withdrawal failed')
                WHEN NEW.state = 'cancelled' THEN 'Withdrawal cancelled by user'
                ELSE 'State changed to ' || NEW.state
            END,
            jsonb_build_object(
                'old_state', OLD.state,
                'new_state', NEW.state,
                'transaction_hash', NEW.transaction_hash,
                'confirmation_count', NEW.confirmation_count
            )
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;