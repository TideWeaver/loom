# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Loom is a Rust-based database interaction tool built as a workspace with two main components:
- `loom` - Core library crate that provides database connectivity and query execution
- `loom-cli` - Command-line interface that uses the loom library

The project uses PostgreSQL as its database backend and provides configuration through YAML files or environment variables.

## Architecture

### Core Library (`loom/`)
- **Config Management**: Supports configuration via YAML files (`config.yml`/`config.yaml`) and environment variables with `LOOM_` prefix
- **Database Pool**: Uses SQLx with PostgreSQL connection pooling
- **Multiple Initialization Patterns**:
  - `Loom::new_from_config()` - From config directory (tries YAML file + env vars)
  - `Loom::new_from_env()` - Environment variables only
  - `Loom::new_from_url()` - Direct database URL
- **Error Handling**: Uses SNAFU for structured error handling
- **DataFusion Integration**: Query engine for Arrow-based data processing
  - `QueryEngine` - Execute SQL queries and return Arrow RecordBatches
  - `PostgresTableProvider` - Custom TableProvider for PostgreSQL (placeholder)

### CLI Tool (`loom-cli/`)
- **Commands**:
  - `exec <query>` - Execute SQL queries
  - `ping` - Test database connectivity
  - `query <sql>` - Execute queries using DataFusion and output as Arrow/JSON/CSV
- **Configuration**: Uses `--config` flag (defaults to `./config.yml`)

## Development Commands

### Building
```bash
cargo build                    # Build all workspace members
cargo build --release         # Release build
```

### Testing
```bash
cargo test                     # Run unit tests
cargo test --test it           # Run integration tests (requires DATABASE_URL)
```

### Code Quality
```bash
cargo fmt                      # Format code (uses rustfmt.toml config)
cargo clippy                   # Lint code
taplo fmt                      # Format TOML files (uses taplo.toml config)
```

### Running the CLI
```bash
cargo run --bin loom ping                           # Test connectivity
cargo run --bin loom exec "SELECT 1"               # Execute query
cargo run --bin loom --config /path/to/config exec "SELECT 1"  # Custom config

# DataFusion queries
cargo run --bin loom query "SELECT 1 as num"       # Output as table (default)
cargo run --bin loom query "SELECT 1 as num" --format json  # Output as JSON
cargo run --bin loom query "SELECT 1 as num" --format csv   # Output as CSV
cargo run --bin loom query "SELECT * FROM generate_series(1,10)" --format csv --output data.csv
```

### Running Examples
```bash
cargo run --example datafusion_query                # DataFusion example
```

## Configuration

### Environment Variables
- `LOOM_DATABASE_URL` - PostgreSQL connection string
- `DATABASE_URL` - Used by integration tests

### Config File Format (YAML)
```yaml
database_url: "postgresql://username:password@localhost/dbname"
```

## Integration Tests

The integration tests (`loom/tests/it/main.rs`) require a PostgreSQL database:
1. Set `DATABASE_URL` environment variable or create `.env` file
2. Tests will create/drop temporary tables for validation
3. Tests include comprehensive error handling validation

## Code Style Configuration

- **Rust Toolchain**: Stable channel with rustfmt, clippy, rust-analyzer
- **Formatting**: 120 character comment width, grouped imports, item-level granularity
- **TOML Formatting**: 80 character width, compact arrays, reordered keys

## DataFusion Integration

The project includes DataFusion integration for Arrow-based query processing:

### Key Components
- `datafusion::QueryEngine` - Main query engine that executes SQL and returns Arrow RecordBatches
- `datafusion::PostgresTableProvider` - Placeholder for PostgreSQL table scanning (not fully implemented)

### Usage Examples
```rust
// As a library
let engine = Arc::new(loom).query_engine().await?;
let batches = engine.query_to_arrow("SELECT 1 as num").await?;

// Via CLI
cargo run --bin loom query "SELECT 1" --format json
cargo run --bin loom query "SELECT * FROM generate_series(1,10)" --format csv --output data.csv
```

### Current Limitations
- PostgreSQL table scanning is not implemented - only DataFusion's built-in SQL functions work
- To query actual PostgreSQL tables, use the `exec` command instead
- Full TableProvider implementation would require datafusion-table-providers or custom implementation