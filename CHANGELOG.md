# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Multi-database support (PostgreSQL and MySQL)
- Export tables to Parquet format
- Import data from Parquet snapshots
- Comprehensive E2E test suite
- GitHub Actions CI/CD pipelines with sccache
- Multi-platform binary releases
- DataFusion query engine integration

### Changed
- Renamed binary from `loom` to `data-loom`
- Updated configuration format to support multiple database types

### Fixed
- Improved error handling across all operations

## [0.0.1] - 2024-01-01

### Added
- Initial release
- PostgreSQL connection pooling
- Basic SQL execution
- Configuration via YAML or environment variables
- CLI interface with multiple commands