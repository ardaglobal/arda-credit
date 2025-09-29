# Create User and Magic Link Script

This script creates a test user and generates a magic link for login testing. The email address can be specified as an argument or defaults to `test@test.com`.

## Usage

### Option 1: Run the shell script with custom email (recommended)
```bash
./create_user_and_magic_link.sh user@example.com
```

### Option 2: Run the shell script with default email
```bash
./create_user_and_magic_link.sh
# Uses test@test.com as the default email
```

### Option 3: Run directly with Cargo
```bash
cargo run -p create_user_and_magic_link -- user@example.com
# Or with default:
cargo run -p create_user_and_magic_link
```

## Environment Variables

- `API_BASE_URL`: Base URL for the API server (default: `http://0.0.0.0:8080`)

## What it does

1. **Creates a user**: Registers the specified email (or `test@test.com` by default) as an investor using the `INVESTOR2025` invite code
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
2. Query the database: `psql $DATABASE_URL -c "SELECT token, state, expires_at FROM magic_links WHERE email = '\$EMAIL' ORDER BY created_at DESC LIMIT 1;"` (replace `\$EMAIL` with the email you used)

3. Visit the magic link URL in your browser
4. You should be authenticated and redirected with a session cookie

**Note**: The default email `test@test.com` will realistically fail since it's not a real email address. Use a valid email address for actual testing.

## Troubleshooting

- **User already exists**: The script handles this gracefully and continues to magic link generation
- **API server not running**: Start with `make up` or `make server`
- **Database issues**: Ensure database is running and migrated with `make initDB`
- **Email not sent**: Check email service configuration, but magic links are still stored in the database
