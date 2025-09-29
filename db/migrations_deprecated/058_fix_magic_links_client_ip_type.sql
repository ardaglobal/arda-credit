-- Fix magic_links table client_ip column type
-- Change from INET to TEXT to match sessions table and avoid type conversion issues

-- Change the client_ip column type from INET to TEXT
ALTER TABLE magic_links ALTER COLUMN client_ip TYPE TEXT;

-- Add a comment to clarify the purpose
COMMENT ON COLUMN magic_links.client_ip IS 'Client IP address as text string for security logging';
