//! Import data from Parquet files back to database

use arrow::array::{
    Array, BooleanArray, Date32Array, Float32Array, Float64Array, Int16Array, Int32Array,
    Int64Array, StringArray, TimestampMicrosecondArray,
};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use bytes::Bytes;
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use snafu::prelude::*;

use crate::parquet::{metadata::*, storage::*};

#[derive(Debug, Snafu)]
pub enum ImportError {
    #[snafu(display("Failed to read metadata: {}", source))]
    ReadMetadata { source: StorageError },

    #[snafu(display("Failed to parse metadata: {}", source))]
    ParseMetadata { source: serde_json::Error },

    #[snafu(display("Failed to read Parquet file: {}", source))]
    ReadParquet {
        source: parquet::errors::ParquetError,
    },

    #[snafu(display("Failed to create table: {}", source))]
    CreateTable { source: sqlx::Error },

    #[snafu(display("Failed to import data: {}", source))]
    ImportData { source: sqlx::Error },

    #[snafu(display("Unsupported database type"))]
    UnsupportedDatabase,

    #[snafu(display("Arrow error: {}", source))]
    Arrow { source: arrow::error::ArrowError },

    #[snafu(display("IO error: {}", source))]
    Io { source: std::io::Error },
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub batch_size: usize,
    pub truncate_before_import: bool,
    pub create_table_if_not_exists: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            batch_size: 10_000,
            truncate_before_import: false,
            create_table_if_not_exists: true,
        }
    }
}

// PostgreSQL Importer
pub struct PostgresImporter {
    pool: sqlx::PgPool,
    options: ImportOptions,
}

impl PostgresImporter {
    pub fn new(pool: sqlx::PgPool, options: ImportOptions) -> Self {
        Self { pool, options }
    }

    async fn create_table_from_metadata(
        &self,
        metadata: &TableMetadata,
    ) -> Result<(), ImportError> {
        let schema_name = metadata.source.schema.as_deref().unwrap_or("public");
        let table_name = &metadata.source.table;

        // Build CREATE TABLE statement
        let mut create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (",
            schema_name, table_name
        );

        for (i, col) in metadata.table_info.columns.iter().enumerate() {
            if i > 0 {
                create_sql.push_str(", ");
            }

            create_sql.push_str(&format!("{} {}", col.name, col.data_type));

            if !col.is_nullable {
                create_sql.push_str(" NOT NULL");
            }

            if let Some(default) = &col.default_value {
                create_sql.push_str(&format!(" DEFAULT {}", default));
            }
        }

        // Add primary key constraint
        let pk_columns: Vec<&str> = metadata
            .table_info
            .columns
            .iter()
            .filter(|col| col.is_primary_key)
            .map(|col| col.name.as_str())
            .collect();

        if !pk_columns.is_empty() {
            create_sql.push_str(&format!(", PRIMARY KEY ({})", pk_columns.join(", ")));
        }

        create_sql.push(')');

        sqlx::query(&create_sql)
            .execute(&self.pool)
            .await
            .context(CreateTableSnafu)?;

        Ok(())
    }

    async fn truncate_table(&self, source: &SourceInfo) -> Result<(), ImportError> {
        let schema_name = source.schema.as_deref().unwrap_or("public");
        let truncate_sql = format!("TRUNCATE TABLE {}.{}", schema_name, source.table);

        sqlx::query(&truncate_sql)
            .execute(&self.pool)
            .await
            .context(ImportDataSnafu)?;

        Ok(())
    }

    async fn import_parquet_data(
        &self,
        parquet_path: &str,
        metadata: &TableMetadata,
        storage_config: &StorageConfig,
    ) -> Result<i64, ImportError> {
        // Read Parquet file
        let parquet_bytes =
            storage_config
                .read_bytes(parquet_path)
                .await
                .map_err(|e| ImportError::Io {
                    source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                })?;

        let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::from(parquet_bytes))
            .context(ReadParquetSnafu)?
            .build()
            .context(ReadParquetSnafu)?;

        let schema_name = metadata.source.schema.as_deref().unwrap_or("public");
        let table_name = &metadata.source.table;

        let mut total_rows = 0i64;

        // Process each batch
        for batch_result in reader {
            let batch = batch_result.map_err(|e| ImportError::Arrow { source: e })?;
            total_rows += self
                .insert_batch(
                    &batch,
                    schema_name,
                    table_name,
                    &metadata.table_info.columns,
                )
                .await?;
        }

        Ok(total_rows)
    }

    async fn insert_batch(
        &self,
        batch: &RecordBatch,
        schema_name: &str,
        table_name: &str,
        columns: &[ColumnMetadata],
    ) -> Result<i64, ImportError> {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return Ok(0);
        }

        // Build INSERT statement
        let column_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        let insert_sql = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            schema_name,
            table_name,
            column_names.join(", "),
            placeholders.join(", ")
        );

        // Convert Arrow arrays to values and insert row by row
        // In a production system, you'd want to batch these inserts
        for row_idx in 0..num_rows {
            let mut query = sqlx::query(&insert_sql);

            for column in batch.columns().iter() {
                query = match column.data_type() {
                    DataType::Int16 => {
                        let array = column.as_any().downcast_ref::<Int16Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i16>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Int32 => {
                        let array = column.as_any().downcast_ref::<Int32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i32>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Int64 => {
                        let array = column.as_any().downcast_ref::<Int64Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i64>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Float32 => {
                        let array = column.as_any().downcast_ref::<Float32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<f32>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Float64 => {
                        let array = column.as_any().downcast_ref::<Float64Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<f64>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Boolean => {
                        let array = column.as_any().downcast_ref::<BooleanArray>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<bool>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Utf8 => {
                        let array = column.as_any().downcast_ref::<StringArray>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<String>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Date32 => {
                        let array = column.as_any().downcast_ref::<Date32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<NaiveDate>)
                        } else {
                            let days = array.value(row_idx);
                            let date = NaiveDate::from_ymd_opt(1970, 1, 1)
                                .unwrap()
                                .checked_add_days(chrono::Days::new(days as u64))
                                .unwrap();
                            query.bind(date)
                        }
                    }
                    DataType::Timestamp(_, _) => {
                        let array = column
                            .as_any()
                            .downcast_ref::<TimestampMicrosecondArray>()
                            .unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<NaiveDateTime>)
                        } else {
                            let micros = array.value(row_idx);
                            let datetime =
                                DateTime::from_timestamp_micros(micros).unwrap().naive_utc();
                            query.bind(datetime)
                        }
                    }
                    _ => query, // Skip unsupported types
                };
            }

            query.execute(&self.pool).await.context(ImportDataSnafu)?;
        }

        Ok(num_rows as i64)
    }
}

#[async_trait::async_trait]
impl crate::parquet::ParquetImporter for PostgresImporter {
    async fn import_snapshot(
        &self,
        snapshot_path: &str,
        storage_config: &StorageConfig,
    ) -> Result<i64, ImportError> {
        // Read metadata
        let metadata_path = format!("{}/metadata.json", snapshot_path);
        let metadata_bytes = storage_config
            .read_bytes(&metadata_path)
            .await
            .context(ReadMetadataSnafu)?;

        let metadata: TableMetadata =
            serde_json::from_slice(&metadata_bytes).context(ParseMetadataSnafu)?;

        // Ensure we're importing to PostgreSQL
        match metadata.source.db_type {
            DatabaseType::PostgreSQL => {}
            _ => return Err(ImportError::UnsupportedDatabase),
        }

        // Create table if needed
        if self.options.create_table_if_not_exists {
            self.create_table_from_metadata(&metadata).await?;
        }

        // Truncate if requested
        if self.options.truncate_before_import {
            self.truncate_table(&metadata.source).await?;
        }

        // Import data
        let parquet_path = format!("{}/data.parquet", snapshot_path);
        self.import_parquet_data(&parquet_path, &metadata, storage_config)
            .await
    }

    async fn list_snapshots(
        &self,
        database: &str,
        table: &str,
        storage_config: &StorageConfig,
    ) -> Result<Vec<String>, ImportError> {
        let base_path = storage_config.get_base_path();
        let table_path = format!("{}/{}_{}", base_path, database, table);

        // List all directories under the table path
        let entries = std::fs::read_dir(&table_path).map_err(|e| ImportError::Io { source: e })?;

        let mut snapshots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| ImportError::Io { source: e })?;
            if entry
                .file_type()
                .map_err(|e| ImportError::Io { source: e })?
                .is_dir()
            {
                if let Some(name) = entry.file_name().to_str() {
                    snapshots.push(name.to_string());
                }
            }
        }

        snapshots.sort();
        snapshots.reverse(); // Most recent first

        Ok(snapshots)
    }
}

// MySQL Importer
pub struct MysqlImporter {
    pool: sqlx::MySqlPool,
    options: ImportOptions,
}

impl MysqlImporter {
    pub fn new(pool: sqlx::MySqlPool, options: ImportOptions) -> Self {
        Self { pool, options }
    }

    async fn create_table_from_metadata(
        &self,
        metadata: &TableMetadata,
    ) -> Result<(), ImportError> {
        let database = metadata
            .source
            .schema
            .as_deref()
            .unwrap_or(&metadata.source.database);
        let table_name = &metadata.source.table;

        // Build CREATE TABLE statement
        let mut create_sql = format!(
            "CREATE TABLE IF NOT EXISTS `{}`.`{}` (",
            database, table_name
        );

        for (i, col) in metadata.table_info.columns.iter().enumerate() {
            if i > 0 {
                create_sql.push_str(", ");
            }

            create_sql.push_str(&format!("`{}` {}", col.name, col.data_type));

            if !col.is_nullable {
                create_sql.push_str(" NOT NULL");
            }

            if let Some(default) = &col.default_value {
                create_sql.push_str(&format!(" DEFAULT {}", default));
            }
        }

        // Add primary key constraint
        let pk_columns: Vec<String> = metadata
            .table_info
            .columns
            .iter()
            .filter(|col| col.is_primary_key)
            .map(|col| format!("`{}`", col.name))
            .collect();

        if !pk_columns.is_empty() {
            create_sql.push_str(&format!(", PRIMARY KEY ({})", pk_columns.join(", ")));
        }

        create_sql.push(')');

        sqlx::query(&create_sql)
            .execute(&self.pool)
            .await
            .context(CreateTableSnafu)?;

        Ok(())
    }

    async fn truncate_table(&self, source: &SourceInfo) -> Result<(), ImportError> {
        let database = source.schema.as_deref().unwrap_or(&source.database);
        let truncate_sql = format!("TRUNCATE TABLE `{}`.`{}`", database, source.table);

        sqlx::query(&truncate_sql)
            .execute(&self.pool)
            .await
            .context(ImportDataSnafu)?;

        Ok(())
    }

    async fn import_parquet_data(
        &self,
        parquet_path: &str,
        metadata: &TableMetadata,
        storage_config: &StorageConfig,
    ) -> Result<i64, ImportError> {
        // Read Parquet file
        let parquet_bytes =
            storage_config
                .read_bytes(parquet_path)
                .await
                .map_err(|e| ImportError::Io {
                    source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                })?;

        let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::from(parquet_bytes))
            .context(ReadParquetSnafu)?
            .build()
            .context(ReadParquetSnafu)?;

        let database = metadata
            .source
            .schema
            .as_deref()
            .unwrap_or(&metadata.source.database);
        let table_name = &metadata.source.table;

        let mut total_rows = 0i64;

        // Process each batch
        for batch_result in reader {
            let batch = batch_result.map_err(|e| ImportError::Arrow { source: e })?;
            total_rows += self
                .insert_batch(&batch, database, table_name, &metadata.table_info.columns)
                .await?;
        }

        Ok(total_rows)
    }

    async fn insert_batch(
        &self,
        batch: &RecordBatch,
        database: &str,
        table_name: &str,
        columns: &[ColumnMetadata],
    ) -> Result<i64, ImportError> {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return Ok(0);
        }

        // Build INSERT statement
        let column_names: Vec<String> = columns.iter().map(|c| format!("`{}`", c.name)).collect();
        let placeholders: Vec<&str> = vec!["?"; columns.len()];

        let insert_sql = format!(
            "INSERT INTO `{}`.`{}` ({}) VALUES ({})",
            database,
            table_name,
            column_names.join(", "),
            placeholders.join(", ")
        );

        // Convert Arrow arrays to values and insert row by row
        for row_idx in 0..num_rows {
            let mut query = sqlx::query(&insert_sql);

            for column in batch.columns().iter() {
                query = match column.data_type() {
                    DataType::Int16 => {
                        let array = column.as_any().downcast_ref::<Int16Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i16>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Int32 => {
                        let array = column.as_any().downcast_ref::<Int32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i32>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Int64 => {
                        let array = column.as_any().downcast_ref::<Int64Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<i64>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Float32 => {
                        let array = column.as_any().downcast_ref::<Float32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<f32>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Float64 => {
                        let array = column.as_any().downcast_ref::<Float64Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<f64>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Boolean => {
                        let array = column.as_any().downcast_ref::<BooleanArray>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<bool>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Utf8 => {
                        let array = column.as_any().downcast_ref::<StringArray>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<String>)
                        } else {
                            query.bind(array.value(row_idx))
                        }
                    }
                    DataType::Date32 => {
                        let array = column.as_any().downcast_ref::<Date32Array>().unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<NaiveDate>)
                        } else {
                            let days = array.value(row_idx);
                            let date = NaiveDate::from_ymd_opt(1970, 1, 1)
                                .unwrap()
                                .checked_add_days(chrono::Days::new(days as u64))
                                .unwrap();
                            query.bind(date)
                        }
                    }
                    DataType::Timestamp(_, _) => {
                        let array = column
                            .as_any()
                            .downcast_ref::<TimestampMicrosecondArray>()
                            .unwrap();
                        if array.is_null(row_idx) {
                            query.bind(None::<NaiveDateTime>)
                        } else {
                            let micros = array.value(row_idx);
                            let datetime =
                                DateTime::from_timestamp_micros(micros).unwrap().naive_utc();
                            query.bind(datetime)
                        }
                    }
                    _ => query, // Skip unsupported types
                };
            }

            query.execute(&self.pool).await.context(ImportDataSnafu)?;
        }

        Ok(num_rows as i64)
    }
}

#[async_trait::async_trait]
impl crate::parquet::ParquetImporter for MysqlImporter {
    async fn import_snapshot(
        &self,
        snapshot_path: &str,
        storage_config: &StorageConfig,
    ) -> Result<i64, ImportError> {
        // Read metadata
        let metadata_path = format!("{}/metadata.json", snapshot_path);
        let metadata_bytes = storage_config
            .read_bytes(&metadata_path)
            .await
            .context(ReadMetadataSnafu)?;

        let metadata: TableMetadata =
            serde_json::from_slice(&metadata_bytes).context(ParseMetadataSnafu)?;

        // Ensure we're importing to MySQL
        match metadata.source.db_type {
            DatabaseType::MySQL => {}
            _ => return Err(ImportError::UnsupportedDatabase),
        }

        // Create table if needed
        if self.options.create_table_if_not_exists {
            self.create_table_from_metadata(&metadata).await?;
        }

        // Truncate if requested
        if self.options.truncate_before_import {
            self.truncate_table(&metadata.source).await?;
        }

        // Import data
        let parquet_path = format!("{}/data.parquet", snapshot_path);
        self.import_parquet_data(&parquet_path, &metadata, storage_config)
            .await
    }

    async fn list_snapshots(
        &self,
        database: &str,
        table: &str,
        storage_config: &StorageConfig,
    ) -> Result<Vec<String>, ImportError> {
        let base_path = storage_config.get_base_path();
        let table_path = format!("{}/{}_{}", base_path, database, table);

        // List all directories under the table path
        let entries = std::fs::read_dir(&table_path).map_err(|e| ImportError::Io { source: e })?;

        let mut snapshots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| ImportError::Io { source: e })?;
            if entry
                .file_type()
                .map_err(|e| ImportError::Io { source: e })?
                .is_dir()
            {
                if let Some(name) = entry.file_name().to_str() {
                    snapshots.push(name.to_string());
                }
            }
        }

        snapshots.sort();
        snapshots.reverse(); // Most recent first

        Ok(snapshots)
    }
}
