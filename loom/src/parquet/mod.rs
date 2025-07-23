//! Parquet export/import functionality

mod exporter;
mod importer;
mod metadata;
mod storage;

pub use exporter::ExportOptions;
pub use importer::ImportOptions;
pub use metadata::{ColumnMetadata, TableMetadata};
pub use storage::{StorageBackend, StorageConfig};

use std::sync::Arc;

// Trait for database-agnostic export operations
#[async_trait::async_trait]
pub trait ParquetExporter: Send + Sync {
    async fn export_table(
        &self,
        database: &str,
        schema: Option<&str>,
        table: &str,
    ) -> Result<TableMetadata, exporter::ExportError>;
}

// Trait for database-agnostic import operations
#[async_trait::async_trait]
pub trait ParquetImporter: Send + Sync {
    async fn import_snapshot(
        &self,
        snapshot_path: &str,
        storage_config: &StorageConfig,
    ) -> Result<i64, importer::ImportError>;

    async fn list_snapshots(
        &self,
        database: &str,
        table: &str,
        storage_config: &StorageConfig,
    ) -> Result<Vec<String>, importer::ImportError>;
}

// Factory functions
pub fn create_postgres_exporter(
    pool: sqlx::PgPool,
    options: ExportOptions,
) -> Arc<dyn ParquetExporter> {
    Arc::new(exporter::PostgresExporter::new(pool, options))
}

pub fn create_mysql_exporter(
    pool: sqlx::MySqlPool,
    options: ExportOptions,
) -> Arc<dyn ParquetExporter> {
    Arc::new(exporter::MysqlExporter::new(pool, options))
}

pub fn create_postgres_importer(
    pool: sqlx::PgPool,
    options: ImportOptions,
) -> Arc<dyn ParquetImporter> {
    Arc::new(importer::PostgresImporter::new(pool, options))
}

pub fn create_mysql_importer(
    pool: sqlx::MySqlPool,
    options: ImportOptions,
) -> Arc<dyn ParquetImporter> {
    Arc::new(importer::MysqlImporter::new(pool, options))
}
