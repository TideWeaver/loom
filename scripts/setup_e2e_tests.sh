#!/bin/bash
# Setup script for E2E tests

echo "Setting up test databases for E2E tests..."

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# PostgreSQL setup
if command -v psql &> /dev/null; then
    echo -e "${GREEN}PostgreSQL found. Setting up test database...${NC}"
    
    # Create test database if it doesn't exist
    createdb loom_test 2>/dev/null || echo "Database loom_test already exists"
    
    # Export the connection URL
    export PG_DATABASE_URL="postgresql://localhost/loom_test"
    echo -e "${GREEN}PostgreSQL test database ready at: $PG_DATABASE_URL${NC}"
else
    echo -e "${YELLOW}PostgreSQL not found. Skipping PostgreSQL tests.${NC}"
fi

# MySQL setup
if command -v mysql &> /dev/null; then
    echo -e "${GREEN}MySQL found. Setting up test database...${NC}"
    
    # Create test database if it doesn't exist
    mysql -u root -e "CREATE DATABASE IF NOT EXISTS loom_test;" 2>/dev/null || {
        echo -e "${YELLOW}Could not create MySQL database. You may need to provide credentials.${NC}"
        echo "Try: mysql -u root -p -e 'CREATE DATABASE loom_test;'"
    }
    
    # Export the connection URL (adjust username/password as needed)
    export MYSQL_DATABASE_URL="mysql://root@localhost/loom_test"
    echo -e "${GREEN}MySQL test database ready at: $MYSQL_DATABASE_URL${NC}"
else
    echo -e "${YELLOW}MySQL not found. Skipping MySQL tests.${NC}"
fi

echo ""
echo "Running E2E tests..."
echo ""

# Run the tests
cargo test --test e2e_export_import -- --nocapture

echo ""
echo "Test setup complete!"