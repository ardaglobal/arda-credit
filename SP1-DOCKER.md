# SP1 Docker Base Image

This document explains how to use the SP1 Docker base image setup to speed up your Docker builds.

## Overview

We've split the Docker setup into two parts:
1. **SP1 Base Image** (`Dockerfile.sp1-base`) - Contains Rust + SP1 toolchain pre-installed
2. **Application Dockerfile** (`Dockerfile`) - Uses the base image and builds your application

This approach significantly reduces build times by avoiding the need to download and install the SP1 toolchain on every build.

## Quick Start

### Step 1: Build the SP1 Base Image

First, build the SP1 base image locally:

```bash
docker build -f Dockerfile.sp1-base -t sp1-base:local .
```

This will create a base image with Rust 1.88.0 and SP1 pre-installed.

### Step 2: Build Your Application

Now you can build your application using the base image:

```bash
docker build -t your-app:latest .
```

The application build will be much faster since SP1 is already installed in the base image.

## Publishing the Base Image

For team collaboration and CI/CD, you should publish the SP1 base image to a Docker registry:

### Option 1: Docker Hub

```bash
# Tag for Docker Hub
docker tag sp1-base:local yourdockerhubusername/sp1-base:1.0

# Push to Docker Hub
docker push yourdockerhubusername/sp1-base:1.0
```

### Option 2: Private Registry (AWS ECR, etc.)

```bash
# Tag for private registry
docker tag sp1-base:local your-registry.com/sp1-base:1.0

# Push to private registry
docker push your-registry.com/sp1-base:1.0
```

### Step 3: Update Application Dockerfile

Once published, update your `Dockerfile` to use the published image:

```dockerfile
# Comment out the local version
# FROM sp1-base:local AS builder

# Uncomment and update with your registry
FROM yourdockerhubusername/sp1-base:1.0 AS builder
```

## Build Performance Comparison

### Before (with inline SP1 installation):
- SP1 installation: ~5-10 minutes (depending on network)
- Application build: ~2-5 minutes
- **Total: ~7-15 minutes**

### After (with SP1 base image):
- Base image pull: ~1-2 minutes (cached after first pull)
- Application build: ~2-5 minutes
- **Total: ~3-7 minutes**

**Performance gain: ~50-60% faster builds**

## Maintenance

### Updating SP1 Version

When SP1 releases a new version:

1. Update the base image:
   ```bash
   docker build -f Dockerfile.sp1-base -t sp1-base:local .
   ```

2. Test with your application:
   ```bash
   docker build -t your-app:test .
   ```

3. If tests pass, publish the new version:
   ```bash
   docker tag sp1-base:local yourdockerhubusername/sp1-base:1.1
   docker push yourdockerhubusername/sp1-base:1.1
   ```

4. Update your application Dockerfile to use the new version.

### Updating Rust Version

To update the Rust version, modify the `FROM` line in `Dockerfile.sp1-base`:

```dockerfile
FROM rust:1.89.0  # Update version here
```

Then rebuild and republish the base image.

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Build and Push SP1 Base Image

on:
  push:
    paths:
      - 'Dockerfile.sp1-base'
  schedule:
    # Rebuild weekly to get security updates
    - cron: '0 0 * * 0'

jobs:
  build-base-image:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Build SP1 base image
        run: docker build -f Dockerfile.sp1-base -t sp1-base:latest .
      
      - name: Login to registry
        run: echo "${{ secrets.DOCKER_PASSWORD }}" | docker login -u "${{ secrets.DOCKER_USERNAME }}" --password-stdin
      
      - name: Push base image
        run: |
          docker tag sp1-base:latest ${{ secrets.DOCKER_USERNAME }}/sp1-base:latest
          docker tag sp1-base:latest ${{ secrets.DOCKER_USERNAME }}/sp1-base:$(date +%Y%m%d)
          docker push ${{ secrets.DOCKER_USERNAME }}/sp1-base:latest
          docker push ${{ secrets.DOCKER_USERNAME }}/sp1-base:$(date +%Y%m%d)
```

## Troubleshooting

### Common Issues

1. **"sp1 command not found"**
   - Make sure you're using the correct base image
   - Verify the PATH is set correctly in the base image

2. **Base image not found**
   - Make sure you've built the base image locally or it exists in your registry
   - Check the image name and tag in your Dockerfile

3. **Slow builds despite using base image**
   - Check if you're pulling the base image from cache
   - Ensure Docker layer caching is working properly

### Verification Commands

To verify the SP1 base image is working correctly:

```bash
# Check if SP1 is installed in the base image
docker run --rm sp1-base:local sp1 --version

# Interactive shell to explore the base image
docker run -it --rm sp1-base:local /bin/bash
```

## Best Practices

1. **Pin versions**: Always use specific tags rather than `latest` in production
2. **Regular updates**: Rebuild the base image regularly for security updates
3. **Multi-architecture**: Consider building for multiple architectures (amd64, arm64)
4. **Size optimization**: The base image is optimized for build speed, not size
5. **Caching**: Leverage Docker's build cache and registry caching for best performance

## File Structure

```
├── Dockerfile.sp1-base    # SP1 base image definition
├── Dockerfile             # Application dockerfile (uses base image)
├── SP1-DOCKER.md         # This documentation
└── ...                   # Your application files
```

## Support

For issues with:
- SP1 installation: Check [SP1 documentation](https://docs.succinct.xyz/)
- Docker builds: Review Docker build logs and check image layers
- Base image problems: Verify the base image builds successfully in isolation