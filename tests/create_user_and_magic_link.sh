#!/bin/bash

# Script to create a user and generate a magic link
# This is a convenience wrapper around the Rust program
#
# Usage:
#   ./create_user_and_magic_link.sh [email]
#
# Arguments:
#   email - Email address to use (default: test@test.com)

set -e

# Get email from argument or use default
EMAIL="${1:-test@test.com}"

echo "🚀 Creating user $EMAIL and generating magic link..."

# Run the Rust program with the email argument
cargo run -p create_user_and_magic_link -- "$EMAIL"

echo ""
echo "🎉 Done! Check the output above for the magic link details."
