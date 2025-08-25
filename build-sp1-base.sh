#!/bin/bash

# Build script for SP1 base Docker image
# This script builds the SP1 base image and optionally pushes it to a registry

set -e

# Configuration
BASE_IMAGE_NAME="sp1-base"
LOCAL_TAG="local"
REGISTRY_TAG=""
PUSH_TO_REGISTRY=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -r, --registry REGISTRY    Push to registry (e.g., dockerhub_username/sp1-base:1.0)"
    echo "  -t, --tag TAG             Local tag for the image (default: local)"
    echo "  -h, --help                Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                                          # Build locally as sp1-base:local"
    echo "  $0 -r username/sp1-base:1.0                # Build and push to Docker Hub"
    echo "  $0 -t latest -r myregistry.com/sp1-base:1.0 # Build as sp1-base:latest and push"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--registry)
            REGISTRY_TAG="$2"
            PUSH_TO_REGISTRY=true
            shift 2
            ;;
        -t|--tag)
            LOCAL_TAG="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Build the base image
echo -e "${GREEN}Building SP1 base image...${NC}"
echo "Local tag: ${BASE_IMAGE_NAME}:${LOCAL_TAG}"

if ! docker build -f Dockerfile.sp1-base -t "${BASE_IMAGE_NAME}:${LOCAL_TAG}" .; then
    echo -e "${RED}Failed to build SP1 base image${NC}"
    exit 1
fi

echo -e "${GREEN}Successfully built ${BASE_IMAGE_NAME}:${LOCAL_TAG}${NC}"

# Verify the image works
echo -e "${YELLOW}Verifying SP1 installation...${NC}"
if docker run --rm "${BASE_IMAGE_NAME}:${LOCAL_TAG}" sp1 --version; then
    echo -e "${GREEN}SP1 verification successful${NC}"
else
    echo -e "${RED}SP1 verification failed${NC}"
    exit 1
fi

# Push to registry if requested
if [[ "$PUSH_TO_REGISTRY" == true ]]; then
    if [[ -z "$REGISTRY_TAG" ]]; then
        echo -e "${RED}Registry tag not specified${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Pushing to registry: ${REGISTRY_TAG}${NC}"
    
    # Tag for registry
    if ! docker tag "${BASE_IMAGE_NAME}:${LOCAL_TAG}" "$REGISTRY_TAG"; then
        echo -e "${RED}Failed to tag image for registry${NC}"
        exit 1
    fi
    
    # Push to registry
    if ! docker push "$REGISTRY_TAG"; then
        echo -e "${RED}Failed to push image to registry${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Successfully pushed ${REGISTRY_TAG}${NC}"
    echo -e "${YELLOW}Don't forget to update your Dockerfile to use: FROM ${REGISTRY_TAG}${NC}"
fi

echo -e "${GREEN}Build complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Build your application: docker build -t your-app ."
echo "2. Or update Dockerfile to use: FROM ${BASE_IMAGE_NAME}:${LOCAL_TAG}"
if [[ "$PUSH_TO_REGISTRY" == true ]]; then
    echo "3. Update Dockerfile to use registry image: FROM ${REGISTRY_TAG}"
fi