#!/bin/bash
# Comprehensive test runner for data-loom

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Data-Loom E2E Test Runner ===${NC}\n"

# Function to wait for database
wait_for_db() {
    local host=$1
    local port=$2
    local name=$3
    local max_attempts=30
    local attempt=1
    
    echo -n "Waiting for $name to be ready..."
    while ! nc -z $host $port 2>/dev/null; do
        if [ $attempt -eq $max_attempts ]; then
            echo -e "\n${RED}Failed to connect to $name after $max_attempts attempts${NC}"
            return 1
        fi
        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done
    echo -e " ${GREEN}Ready!${NC}"
    return 0
}

# Check if Docker Compose is available
if command -v docker-compose &> /dev/null || command -v docker &> /dev/null && docker compose version &> /dev/null; then
    echo -e "${GREEN}Docker Compose found. Starting test databases...${NC}"
    
    # Use docker compose or docker-compose based on what's available
    if command -v docker-compose &> /dev/null; then
        DOCKER_COMPOSE="docker-compose"
    else
        DOCKER_COMPOSE="docker compose"
    fi
    
    # Start databases
    $DOCKER_COMPOSE -f docker-compose.test.yml up -d
    
    # Wait for databases to be ready
    wait_for_db localhost 5432 PostgreSQL
    wait_for_db localhost 3306 MySQL
    
    # Give databases a moment to fully initialize
    sleep 2
    
    # Set environment variables
    export PG_DATABASE_URL="postgresql://postgres:postgres@localhost:5432/loom_test"
    export MYSQL_DATABASE_URL="mysql://root:mysql@localhost:3306/loom_test"
    
    echo -e "\n${GREEN}Test databases are ready!${NC}"
else
    echo -e "${YELLOW}Docker Compose not found. Using local databases if available.${NC}"
    
    # Try to use local databases
    if command -v psql &> /dev/null; then
        export PG_DATABASE_URL="postgresql://localhost/loom_test"
    fi
    
    if command -v mysql &> /dev/null; then
        export MYSQL_DATABASE_URL="mysql://root@localhost/loom_test"
    fi
fi

# Run unit tests first
echo -e "\n${BLUE}Running unit tests...${NC}"
cargo test --lib

# Run integration tests
echo -e "\n${BLUE}Running integration tests...${NC}"
if [ -n "$PG_DATABASE_URL" ]; then
    DATABASE_URL=$PG_DATABASE_URL cargo test --test it
else
    echo -e "${YELLOW}Skipping integration tests - no PostgreSQL database available${NC}"
fi

# Run E2E tests
echo -e "\n${BLUE}Running E2E export/import tests...${NC}"
cargo test --test e2e_export_import -- --test-threads=1 --nocapture

# Run CLI tests
echo -e "\n${BLUE}Testing CLI commands...${NC}"

# Test binary exists
if cargo build --bin data-loom 2>/dev/null; then
    echo -e "${GREEN}✓ Binary 'data-loom' builds successfully${NC}"
    
    # Test help command
    if cargo run --bin data-loom -- --help &>/dev/null; then
        echo -e "${GREEN}✓ Help command works${NC}"
    fi
    
    # Test with PostgreSQL if available
    if [ -n "$PG_DATABASE_URL" ]; then
        # Create test config
        cat > test-config.yml <<EOF
database:
  type: postgres
  url: "$PG_DATABASE_URL"
storage:
  type: local
  path: "./test-snapshots"
EOF
        
        # Test ping
        if cargo run --bin data-loom -- --config test-config.yml ping &>/dev/null; then
            echo -e "${GREEN}✓ PostgreSQL ping works${NC}"
        fi
        
        # Clean up
        rm -f test-config.yml
    fi
    
    # Test with MySQL if available
    if [ -n "$MYSQL_DATABASE_URL" ]; then
        # Create test config
        cat > test-config-mysql.yml <<EOF
database:
  type: mysql
  url: "$MYSQL_DATABASE_URL"
storage:
  type: local
  path: "./test-snapshots"
EOF
        
        # Test ping
        if cargo run --bin data-loom -- --config test-config-mysql.yml ping &>/dev/null; then
            echo -e "${GREEN}✓ MySQL ping works${NC}"
        fi
        
        # Clean up
        rm -f test-config-mysql.yml
    fi
else
    echo -e "${RED}✗ Failed to build data-loom binary${NC}"
fi

# Cleanup test snapshots
rm -rf test-snapshots

# Stop Docker containers if we started them
if [ -n "$DOCKER_COMPOSE" ]; then
    echo -e "\n${BLUE}Stopping test databases...${NC}"
    $DOCKER_COMPOSE -f docker-compose.test.yml down -v
fi

echo -e "\n${GREEN}All tests completed!${NC}"