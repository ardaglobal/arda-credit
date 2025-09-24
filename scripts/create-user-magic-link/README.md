# Create User and Magic Link Script

This script creates a test user with email `matt@arda.xyz` and generates a magic link for login testing.

## Usage

### Option 1: Run the shell script (recommended)
```bash
./create_user_and_magic_link.sh
```

### Option 2: Run directly with Cargo
```bash
cargo run -p create_user_and_magic_link
```

## Environment Variables

- `API_BASE_URL`: Base URL for the API server (default: `http://0.0.0.0:8080`)

## What it does

1. **Creates a user**: Registers `matt@arda.xyz` as an investor using the `INVESTOR2025` invite code
2. **Requests magic link**: Sends a magic link request to the API for email authentication
3. **Provides instructions**: Shows how to access the magic link from email or database

## Prerequisites

- The API server must be running on `http://0.0.0.0:8080` (or set `API_BASE_URL`)
- Database must be available and migrated
- Email service should be configured for magic links to actually be sent

## Output

The script will output:
- User registration confirmation with user ID
- Magic link request confirmation
- Instructions for accessing the magic link
- Database query to manually retrieve the magic link token

## Testing the Magic Link Flow

After running the script:

1. Check your email for the magic link, OR
2. Query the database: `psql $DATABASE_URL -c "SELECT token, state, expires_at FROM magic_links WHERE email = 'matt@arda.xyz' ORDER BY created_at DESC LIMIT 1;"`

3. Visit the magic link URL in your browser
4. You should be authenticated and redirected with a session cookie

## Troubleshooting

- **User already exists**: The script handles this gracefully and continues to magic link generation
- **API server not running**: Start with `make up` or `make server`
- **Database issues**: Ensure database is running and migrated with `make initDB`
- **Email not sent**: Check email service configuration, but magic links are still stored in the database
