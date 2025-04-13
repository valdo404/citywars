#!/bin/bash

# Add PostgreSQL binaries to PATH
export PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH"

# Default PostgreSQL connection parameters
DB_URL=${1:-"jdbc:postgresql://localhost:5432/clickplanet"}
DB_USER=${2:-"laurentvaldes"}
DB_PASSWORD=${3:-""}

# Extract database name, host, and port from JDBC URL
DB_HOST=$(echo $DB_URL | sed -n 's/.*:\/\/\([^:]*\).*/\1/p')
DB_PORT=$(echo $DB_URL | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
DB_NAME=$(echo $DB_URL | sed -n 's/.*\/\([^?]*\).*/\1/p')

echo "Creating tables in PostgreSQL database:"
echo "Database: $DB_NAME"
echo "Host: $DB_HOST"
echo "Port: $DB_PORT"
echo "User: $DB_USER"

# Run the SQL script to create tables
PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f create-tables.sql

if [ $? -eq 0 ]; then
  echo "Tables created successfully!"
else
  echo "Error creating tables. Please check your PostgreSQL connection parameters."
  exit 1
fi
