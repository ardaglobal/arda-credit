-- Add 2026 invite codes for investor, originator, and admin user types
-- These codes expire at the end of December 2026

INSERT INTO invite_codes (
    code,
    description,
    max_uses,
    expires_at,
    user_type,
    is_active
) VALUES
    (
        'INVESTOR2026',
        'Investor access invite code - expires December 31st, 2026',
        1000,
        '2026-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'investor',
        true
    ),
    (
        'ORIGINATOR2026',
        'Originator access invite code - expires December 31st, 2026',
        1000,
        '2026-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'originator',
        true
    ),
    (
        'ADMIN2026',
        'Admin access invite code - expires December 31st, 2026',
        1000,
        '2026-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'admin',
        true
    )
ON CONFLICT (code) DO NOTHING;

