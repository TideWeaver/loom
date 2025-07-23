# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Loom is a Rust-based database migration tool built as a workspace with two main components:
- `loom` - Core library crate that provides database connectivity, query execution, and data export/import functionality
- `loom-cli` (renamed to `data-loom`) - Command-line interface that uses the loom library

The project supports both PostgreSQL and MySQL databases and provides data export/import functionality through Parquet files.

## Architecture

### Core Library (`loom/`)
- **Config Management**: Supports configuration via YAML files (`config.yml`/`config.yaml`) and environment variables with `LOOM_` prefix
- **Database Support**: Both PostgreSQL and MySQL with connection pooling via SQLx
- **Multiple Initialization Patterns**:
  - `Loom::new_from_config()` - From config directory (tries YAML file + env vars)
  - `Loom::new_from_env()` - Environment variables only
  - `Loom::new_from_url()` - Direct database URL with database type
- **Error Handling**: Uses SNAFU for structured error handling
- **DataFusion Integration**: Query engine for Arrow-based data processing
- **Export/Import**: Full table export to Parquet format and import back to database

### CLI Tool (`loom-cli/`)
- **Binary Name**: `data-loom`
- **Commands**:
  - `exec <query>` - Execute SQL queries
  - `ping` - Test database connectivity
  - `query <sql>` - Execute queries using DataFusion and output as Arrow/JSON/CSV
  - `export <database> <table>` - Export table to Parquet format
  - `import <database> <table>` - Import data from Parquet snapshot
  - `list-snapshots <database> <table>` - List available snapshots
- **Configuration**: Uses `--config` flag (defaults to `./config.yml`)

## Development Commands

### Building
```bash
cargo build                    # Build all workspace members
cargo build --release         # Release build
cargo build --bin data-loom    # Build just the CLI
```

### Testing
```bash
cargo test                     # Run unit tests
cargo test --test it           # Run integration tests (requires DATABASE_URL)
cargo test --test e2e_export_import  # Run E2E tests (requires PG_DATABASE_URL or MYSQL_DATABASE_URL)

# Run all tests with Docker
./scripts/run_all_tests.sh     # Comprehensive test suite with Docker databases
```

### Code Quality
```bash
cargo fmt                      # Format code (uses rustfmt.toml config)
cargo clippy                   # Lint code
taplo fmt                      # Format TOML files (uses taplo.toml config)
```

### Running the CLI
```bash
# Basic commands
cargo run --bin data-loom ping
cargo run --bin data-loom exec "SELECT 1"

# Export/Import
cargo run --bin data-loom export mydb users
cargo run --bin data-loom import mydb users
cargo run --bin data-loom list-snapshots mydb users

# DataFusion queries
cargo run --bin data-loom query "SELECT 1 as num" --format json
```

## Configuration

### Environment Variables
- `LOOM_DATABASE__TYPE` - Database type: "postgres" or "mysql"
- `LOOM_DATABASE__URL` - Database connection string
- `DATABASE_URL` - Used by integration tests
- `PG_DATABASE_URL` - PostgreSQL URL for E2E tests
- `MYSQL_DATABASE_URL` - MySQL URL for E2E tests

### Config File Format (YAML)
```yaml
database:
  type: postgres  # or mysql
  url: "postgresql://username:password@localhost/dbname"
  
storage:
  type: local
  path: "./data/snapshots"
```

## Database Support

### PostgreSQL
- Full support for common data types
- Schema-aware operations
- Supports: smallint, integer, bigint, real, double precision, boolean, text, varchar, date, timestamp, numeric, decimal, bytea, uuid, json, jsonb

### MySQL
- Full support for common data types
- Database-scoped operations
- Supports: tinyint, smallint, mediumint, int, bigint, float, double, boolean, char, varchar, text, date, datetime, timestamp, decimal, numeric, binary, blob, json

## Export/Import Features

### Export
- Exports entire tables to Parquet format
- Preserves data types and null values
- Creates timestamped snapshots
- Stores metadata alongside data
- Supports batch processing for large tables

### Import
- Recreates tables from metadata if needed
- Supports truncate before import
- Handles all exported data types
- Preserves primary key constraints

### Snapshot Structure
```
data/snapshots/
├── database_table/
│   └── 2024-01-01_10-00-00/
│       ├── data.parquet
│       └── metadata.json
```

## Testing Strategy

### Unit Tests
- Core library functionality
- Configuration parsing
- Error handling

### Integration Tests
- Database connectivity
- Basic SQL operations
- Requires `DATABASE_URL` environment variable

### E2E Tests
- Complete export/import cycles
- Various data types testing
- Large table handling
- Empty table handling
- Requires database URLs:
  - `PG_DATABASE_URL` or `DATABASE_URL` for PostgreSQL
  - `MYSQL_DATABASE_URL` for MySQL

### Docker Test Environment
```bash
docker-compose -f docker-compose.test.yml up -d
export PG_DATABASE_URL="postgresql://postgres:postgres@localhost:5432/loom_test"
export MYSQL_DATABASE_URL="mysql://root:mysql@localhost:3306/loom_test"
cargo test
```