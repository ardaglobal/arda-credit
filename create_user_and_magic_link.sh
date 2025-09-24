#!/bin/bash

# Script to create a user with matt@arda.xyz and generate a magic link
# This is a convenience wrapper around the Rust program

set -e

echo "🚀 Creating user matt@arda.xyz and generating magic link..."

# Run the Rust program
cargo run -p create_user_and_magic_link "$@"

echo ""
echo "🎉 Done! Check the output above for the magic link details."
