# Data-Loom

A high-performance database migration and data export/import tool written in Rust. Data-Loom supports both PostgreSQL and MySQL databases and provides efficient data export to Parquet format for archival, analysis, and migration purposes.

## Features

- 🚀 **High Performance**: Built with Rust for maximum speed and efficiency
- 🗄️ **Multi-Database Support**: Works with both PostgreSQL and MySQL
- 📦 **Parquet Export/Import**: Export tables to efficient Parquet format
- 🔄 **Full Data Preservation**: Maintains data types, nulls, and constraints
- 📊 **DataFusion Integration**: Query data using SQL with Apache Arrow
- 🛡️ **Type Safe**: Leverages Rust's type system for reliability
- 📁 **Snapshot Management**: Timestamped snapshots for version control
- ☁️ **Flexible Storage**: Supports local filesystem and S3

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/data-loom.git
cd data-loom

# Build the project
cargo build --release

# The binary will be available at target/release/data-loom
```

## Quick Start

### Configuration

Create a `config.yml` file:

```yaml
# For PostgreSQL
database:
  type: postgres
  url: "postgresql://user:password@localhost:5432/mydb"

# For MySQL
database:
  type: mysql
  url: "mysql://user:password@localhost:3306/mydb"

storage:
  type: local
  path: "./data/snapshots"
```

### Basic Commands

```bash
# Test database connection
data-loom ping

# Execute SQL query
data-loom exec "SELECT COUNT(*) FROM users"

# Export a table
data-loom export mydb users

# Import from snapshot
data-loom import mydb users

# List available snapshots
data-loom list-snapshots mydb users
```

## Export/Import Usage

### Exporting Data

Export a complete table to Parquet format:

```bash
# Export with schema (PostgreSQL)
data-loom export mydb public.users

# Export without schema (MySQL or default schema)
data-loom export mydb users
```

This creates a timestamped snapshot:
```
data/snapshots/
└── mydb_users/
    └── 2024-01-15_10-30-45/
        ├── data.parquet
        └── metadata.json
```

### Importing Data

Import data back to the database:

```bash
# Import latest snapshot
data-loom import mydb users

# Import specific snapshot
data-loom import mydb users --snapshot 2024-01-15_10-30-45

# Import with truncate
data-loom import mydb users --truncate

# Import without creating table
data-loom import mydb users --no-create-if-not-exists
```

## DataFusion Queries

Use SQL to query data with Apache Arrow:

```bash
# Query with table output
data-loom query "SELECT * FROM generate_series(1, 10)"

# Export to CSV
data-loom query "SELECT * FROM generate_series(1, 100)" --format csv --output data.csv

# Output as JSON
data-loom query "SELECT 1 as id, 'test' as name" --format json
```

## Supported Data Types

### PostgreSQL
- Numeric: smallint, integer, bigint, real, double precision, numeric, decimal
- Text: text, varchar, char
- Boolean: boolean
- Temporal: date, timestamp, timestamp with time zone
- Binary: bytea
- JSON: json, jsonb
- Other: uuid

### MySQL
- Numeric: tinyint, smallint, mediumint, int, bigint, float, double, decimal
- Text: char, varchar, text, tinytext, mediumtext, longtext
- Boolean: boolean, bit
- Temporal: date, datetime, timestamp
- Binary: binary, varbinary, blob, tinyblob, mediumblob, longblob
- JSON: json

## Storage Configuration

### Local Storage
```yaml
storage:
  type: local
  path: "./data/snapshots"
```

### S3 Storage
```yaml
storage:
  type: s3
  bucket: "my-loom-snapshots"
  prefix: "snapshots"
  region: "us-east-1"  # optional
```

## Development

### Building from Source

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with Docker test environment
./scripts/run_all_tests.sh
```

### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires DATABASE_URL)
DATABASE_URL=postgresql://localhost/test cargo test --test it

# E2E tests
PG_DATABASE_URL=postgresql://localhost/test cargo test --test e2e_export_import

# Full test suite with Docker
docker-compose -f docker-compose.test.yml up -d
./scripts/run_all_tests.sh
```

## Architecture

Data-Loom consists of two main components:

1. **Core Library (`loom`)**: Handles database operations, export/import logic
2. **CLI (`data-loom`)**: Command-line interface for user interaction

The export process:
1. Reads table metadata (columns, types, constraints)
2. Streams data in configurable batches
3. Converts to Apache Arrow format
4. Writes to Parquet files
5. Saves metadata for reconstruction

The import process:
1. Reads metadata from snapshot
2. Creates table if needed (matching original schema)
3. Reads Parquet data in batches
4. Inserts data preserving types and nulls

## Use Cases

- **Database Migration**: Migrate data between different database systems
- **Backup & Archive**: Create efficient backups in Parquet format
- **Data Analysis**: Export data for analysis in data science tools
- **Development**: Copy production data to development environments
- **Testing**: Create reproducible test datasets

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [Apache Arrow](https://arrow.apache.org/) - Columnar memory format
- [DataFusion](https://arrow.apache.org/datafusion/) - SQL query engine
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- [Parquet](https://parquet.apache.org/) - Columnar storage format