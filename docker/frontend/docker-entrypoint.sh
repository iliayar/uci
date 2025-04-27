#!/bin/sh
set -e

# Replace environment variables in JavaScript files
find /usr/share/nginx/html -type f -name "*.js" -exec sed -i "s|UCI_BASE_URL_PLACEHOLDER|${UCI_BASE_URL}|g" {} \;
find /usr/share/nginx/html -type f -name "*.js" -exec sed -i "s|UCI_WS_BASE_URL_PLACEHOLDER|${UCI_WS_BASE_URL}|g" {} \;

# Execute the passed command (usually nginx -g daemon off;)
exec "$@"