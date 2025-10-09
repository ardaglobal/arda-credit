# Frontend App Dockerfile
# This Dockerfile is a placeholder for the Arda Credit frontend application
# When the frontend code is added to the `app/` directory, this will build and serve it

# Build stage - for React/Vite/Next.js apps
FROM node:20-alpine AS builder

WORKDIR /app

# Copy package files (when they exist)
# COPY app/package*.json ./
# RUN npm ci

# Copy source code (when it exists)
# COPY app/ ./
# RUN npm run build

# Production stage - serve with nginx
FROM nginx:alpine

# Copy nginx configuration
# COPY app/nginx.conf /etc/nginx/conf.d/default.conf

# Copy built assets from builder (when available)
# COPY --from=builder /app/dist /usr/share/nginx/html

# For now, create a simple index.html placeholder
RUN echo '<html><body><h1>Arda Credit Frontend - Coming Soon</h1><p>The frontend application will be deployed here.</p></body></html>' > /usr/share/nginx/html/index.html

# Create nginx config for API proxying
RUN echo 'server { \
    listen 80; \
    server_name localhost; \
    \
    location / { \
        root /usr/share/nginx/html; \
        try_files $uri $uri/ /index.html; \
    } \
    \
    location /api { \
        proxy_pass http://arda-credit-server:8080; \
        proxy_http_version 1.1; \
        proxy_set_header Upgrade $http_upgrade; \
        proxy_set_header Connection "upgrade"; \
        proxy_set_header Host $host; \
        proxy_set_header X-Real-IP $remote_addr; \
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for; \
        proxy_set_header X-Forwarded-Proto $scheme; \
    } \
}' > /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
