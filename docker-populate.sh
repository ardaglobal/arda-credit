#!/bin/bash
# Docker Population Script Helper
# 
# This script provides easy commands to populate deal data from within
# a Docker container or deployed instance.

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Helper function for colored output
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
    exit 1
}

info() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

# Check if DATABASE_URL is set
check_database_url() {
    if [[ -z "$DATABASE_URL" ]]; then
        error "DATABASE_URL environment variable is not set"
    fi
    log "Using DATABASE_URL: ${DATABASE_URL}"
}

# Display usage information
usage() {
    cat << EOF
Docker Deal Population Script

USAGE:
    $0 [COMMAND] [OPTIONS]

COMMANDS:
    deals-random [COUNT]           Generate random deals (default: 25)
    deals-user EMAIL [COUNT]       Generate deals for specific user (default: 25)
    deals-demo                     Generate demo data (8 deals)
    deals-frontend                 Generate frontend test data (50 deals)
    deals-clear                    Clear existing deal data
    legacy                         Run legacy populate_test_data script
    help                           Show this help message

OPTIONS:
    --clear                        Clear existing data before generating (deals commands only)
    --debug                        Enable debug output

EXAMPLES:
    # Generate 30 random deals
    $0 deals-random 30
    
    # Generate deals for a specific user
    $0 deals-user alice@company.com 20
    
    # Generate demo data with cleanup
    $0 deals-demo --clear
    
    # Generate frontend test data
    $0 deals-frontend
    
    # Clear all deal data
    $0 deals-clear

ENVIRONMENT:
    DATABASE_URL                   PostgreSQL connection string (required)
    RUST_LOG                      Set to 'debug' for verbose output

EOF
}

# Main command processing
case "${1:-help}" in
    "deals-random")
        check_database_url
        COUNT=${2:-25}
        CLEAR_FLAG=""
        
        # Check for --clear flag
        if [[ "$3" == "--clear" ]] || [[ "$2" == "--clear" ]]; then
            CLEAR_FLAG="--clear"
            if [[ "$2" == "--clear" ]]; then
                COUNT=25
            fi
        fi
        
        log "Generating $COUNT random deals${CLEAR_FLAG:+ (with cleanup)}"
        ./populate_deal_data $CLEAR_FLAG --count $COUNT
        ;;
        
    "deals-user")
        if [[ -z "$2" ]]; then
            error "Email address is required for user mode. Usage: $0 deals-user EMAIL [COUNT]"
        fi
        
        check_database_url
        EMAIL=$2
        COUNT=${3:-25}
        CLEAR_FLAG=""
        
        # Check for --clear flag
        if [[ "$4" == "--clear" ]] || [[ "$3" == "--clear" ]]; then
            CLEAR_FLAG="--clear"
            if [[ "$3" == "--clear" ]]; then
                COUNT=25
            fi
        fi
        
        log "Generating $COUNT deals for user: $EMAIL${CLEAR_FLAG:+ (with cleanup)}"
        ./populate_deal_data $CLEAR_FLAG --email "$EMAIL" --count $COUNT
        ;;
        
    "deals-demo")
        check_database_url
        CLEAR_FLAG=""
        
        if [[ "$2" == "--clear" ]]; then
            CLEAR_FLAG="--clear"
        fi
        
        log "Generating demo data (8 deals)${CLEAR_FLAG:+ (with cleanup)}"
        ./populate_deal_data $CLEAR_FLAG --count 8
        ;;
        
    "deals-frontend")
        check_database_url
        CLEAR_FLAG=""
        
        if [[ "$2" == "--clear" ]]; then
            CLEAR_FLAG="--clear"
        fi
        
        log "Generating frontend test data (50 deals)${CLEAR_FLAG:+ (with cleanup)}"
        ./populate_deal_data $CLEAR_FLAG --count 50
        ;;
        
    "deals-clear")
        check_database_url
        warn "This will clear all existing deal data!"
        log "Clearing deal data..."
        ./populate_deal_data --clear --count 0
        ;;
        
    "legacy")
        check_database_url
        log "Running legacy populate_test_data script"
        ./populate_test_data
        ;;
        
    "help"|"--help"|"-h")
        usage
        ;;
        
    *)
        error "Unknown command: $1. Use '$0 help' for usage information."
        ;;
esac

log "Operation completed successfully!"

# Show helpful next steps
info ""
info "Next steps:"
info "  • Test the API: curl http://localhost:8080/api/v1/deals?limit=10"
info "  • View Swagger UI: http://localhost:8080/swagger-ui/"
info "  • Check server logs for any issues"
