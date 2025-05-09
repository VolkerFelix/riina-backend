#!/bin/bash
set -e

if [ "$APP_ENVIRONMENT" = "production" ]; then
  echo "Production environment detected. Running database migrations..."
  if [ -n "$DATABASE_URL" ]; then
    DATABASE_URL="$DATABASE_URL" sqlx migrate run
  else
    echo "ERROR: DATABASE_URL is not set. Migrations cannot be run."
  fi
else
  echo "Non-production environment. Skipping automatic migrations."
fi

echo "Starting application..."
exec areum-backend