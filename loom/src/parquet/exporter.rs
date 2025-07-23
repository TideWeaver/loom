//! Export database tables to Parquet format

use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, Date32Array, Float32Array, Float64Array, Int16Array, Int32Array,
    Int64Array, StringBuilder, TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{NaiveDate, NaiveDateTime};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::{WriterProperties, WriterVersion};
use snafu::prelude::*;
use sqlx::Row;

use crate::parquet::{metadata::*, storage::*};

#[derive(Debug, Snafu)]
pub enum ExportError {
    #[snafu(display("Failed to query table metadata: {}", source))]
    MetadataQuery { source: sqlx::Error },

    #[snafu(display("Failed to export data: {}", source))]
    DataExport { source: sqlx::Error },

    #[snafu(display("Failed to write Parquet file: {}", source))]
    ParquetWrite {
        source: parquet::errors::ParquetError,
    },

    #[snafu(display("Storage error: {}", source))]
    Storage { source: StorageError },

    #[snafu(display("Arrow error: {}", source))]
    Arrow { source: arrow::error::ArrowError },

    #[snafu(display("Unsupported data type: {}", data_type))]
    UnsupportedDataType { data_type: String },
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub batch_size: usize,
    pub row_group_size: usize,
    pub compression: String,
    pub storage_config: StorageConfig,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            batch_size: 10_000,
            row_group_size: 100_000,
            compression: "snappy".to_string(),
            storage_config: StorageConfig::default(),
        }
    }
}

// PostgreSQL Exporter
pub struct PostgresExporter {
    pool: sqlx::PgPool,
    options: ExportOptions,
}

impl PostgresExporter {
    pub fn new(pool: sqlx::PgPool, options: ExportOptions) -> Self {
        Self { pool, options }
    }

    async fn get_table_columns(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<Vec<ColumnMetadata>, ExportError> {
        let schema_name = schema.unwrap_or("public");

        let query = r#"
            SELECT 
                c.column_name,
                c.data_type,
                c.is_nullable,
                c.column_default,
                CASE 
                    WHEN pk.column_name IS NOT NULL THEN true 
                    ELSE false 
                END as is_primary_key
            FROM information_schema.columns c
            LEFT JOIN (
                SELECT ku.column_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage ku
                    ON tc.constraint_name = ku.constraint_name
                    AND tc.table_schema = ku.table_schema
                WHERE tc.constraint_type = 'PRIMARY KEY'
                    AND tc.table_schema = $1
                    AND tc.table_name = $2
            ) pk ON c.column_name = pk.column_name
            WHERE c.table_schema = $1 
                AND c.table_name = $2
            ORDER BY c.ordinal_position
        "#;

        let rows = sqlx::query(query)
            .bind(schema_name)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .context(MetadataQuerySnafu)?;

        let mut columns = Vec::new();
        for row in rows {
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let is_nullable: String = row.get("is_nullable");
            let column_default: Option<String> = row.get("column_default");
            let is_primary_key: bool = row.get("is_primary_key");

            columns.push(ColumnMetadata {
                name: column_name,
                data_type,
                is_nullable: is_nullable == "YES",
                default_value: column_default,
                is_primary_key,
            });
        }

        Ok(columns)
    }

    fn pg_type_to_arrow(pg_type: &str) -> Result<DataType, ExportError> {
        match pg_type {
            "smallint" => Ok(DataType::Int16),
            "integer" => Ok(DataType::Int32),
            "bigint" => Ok(DataType::Int64),
            "real" => Ok(DataType::Float32),
            "double precision" => Ok(DataType::Float64),
            "boolean" => Ok(DataType::Boolean),
            "text" | "character varying" | "character" => Ok(DataType::Utf8),
            "date" => Ok(DataType::Date32),
            "timestamp without time zone" => Ok(DataType::Timestamp(TimeUnit::Microsecond, None)),
            "timestamp with time zone" => Ok(DataType::Timestamp(
                TimeUnit::Microsecond,
                Some("UTC".into()),
            )),
            "numeric" | "decimal" => Ok(DataType::Decimal128(38, 10)), // Default precision/scale
            "bytea" => Ok(DataType::Binary),
            "uuid" => Ok(DataType::Utf8), // Store UUID as string
            "json" | "jsonb" => Ok(DataType::Utf8), // Store JSON as string
            _ => Err(ExportError::UnsupportedDataType {
                data_type: pg_type.to_string(),
            }),
        }
    }

    fn create_arrow_schema(&self, columns: &[ColumnMetadata]) -> Result<Schema, ExportError> {
        let fields: Result<Vec<_>, _> = columns
            .iter()
            .map(|col| {
                let data_type = Self::pg_type_to_arrow(&col.data_type)?;
                Ok(Field::new(&col.name, data_type, col.is_nullable))
            })
            .collect();

        Ok(Schema::new(fields?))
    }

    async fn export_table_data(
        &self,
        schema: Option<&str>,
        table: &str,
        output_path: &str,
        arrow_schema: Arc<Schema>,
    ) -> Result<i64, ExportError> {
        let schema_name = schema.unwrap_or("public");
        let query = format!("SELECT * FROM {}.{}", schema_name, table);

        // Create Parquet writer
        let props = WriterProperties::builder()
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();

        let file = std::fs::File::create(output_path).map_err(|e| ExportError::Storage {
            source: StorageError::WriteFile {
                path: output_path.to_string(),
                source: e,
            },
        })?;

        let mut writer = ArrowWriter::try_new(file, arrow_schema.clone(), Some(props))
            .context(ParquetWriteSnafu)?;

        // Export data in batches
        let mut total_rows = 0i64;
        let mut rows = sqlx::query(&query).fetch(&self.pool);

        use futures::StreamExt;
        let mut batch_data: Vec<Vec<Option<String>>> = Vec::new();

        while let Some(row) = rows.next().await {
            let row = row.context(DataExportSnafu)?;

            // Convert row to string representations for simplicity
            let mut row_data = Vec::new();
            for (i, field) in arrow_schema.fields().iter().enumerate() {
                let value: Option<String> = match field.data_type() {
                    DataType::Int16 => row
                        .try_get::<Option<i16>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Int32 => row
                        .try_get::<Option<i32>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Int64 => row
                        .try_get::<Option<i64>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Float32 => row
                        .try_get::<Option<f32>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Float64 => row
                        .try_get::<Option<f64>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Boolean => row
                        .try_get::<Option<bool>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Utf8 => row.try_get::<Option<String>, _>(i).ok().flatten(),
                    DataType::Date32 => row
                        .try_get::<Option<NaiveDate>, _>(i)
                        .ok()
                        .flatten()
                        .map(|d| {
                            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                            (d.signed_duration_since(epoch).num_days()) as i32
                        })
                        .map(|v| v.to_string()),
                    DataType::Timestamp(_, _) => row
                        .try_get::<Option<NaiveDateTime>, _>(i)
                        .ok()
                        .flatten()
                        .map(|dt| dt.and_utc().timestamp_micros())
                        .map(|v| v.to_string()),
                    _ => None,
                };
                row_data.push(value);
            }

            batch_data.push(row_data);

            if batch_data.len() >= self.options.batch_size {
                let batch = self.create_record_batch(&batch_data, arrow_schema.clone())?;
                writer.write(&batch).context(ParquetWriteSnafu)?;
                total_rows += batch_data.len() as i64;
                batch_data.clear();
            }
        }

        // Write remaining data
        if !batch_data.is_empty() {
            let batch = self.create_record_batch(&batch_data, arrow_schema)?;
            writer.write(&batch).context(ParquetWriteSnafu)?;
            total_rows += batch_data.len() as i64;
        }

        writer.close().context(ParquetWriteSnafu)?;

        Ok(total_rows)
    }

    fn create_record_batch(
        &self,
        data: &[Vec<Option<String>>],
        schema: Arc<Schema>,
    ) -> Result<RecordBatch, ExportError> {
        let mut columns: Vec<ArrayRef> = Vec::new();

        for (field_idx, field) in schema.fields().iter().enumerate() {
            let column_data: Vec<Option<&str>> =
                data.iter().map(|row| row[field_idx].as_deref()).collect();

            let array: ArrayRef = match field.data_type() {
                DataType::Int16 => {
                    let values: Vec<Option<i16>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int16Array::from(values))
                }
                DataType::Int32 => {
                    let values: Vec<Option<i32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int32Array::from(values))
                }
                DataType::Int64 => {
                    let values: Vec<Option<i64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int64Array::from(values))
                }
                DataType::Float32 => {
                    let values: Vec<Option<f32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Float32Array::from(values))
                }
                DataType::Float64 => {
                    let values: Vec<Option<f64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Float64Array::from(values))
                }
                DataType::Boolean => {
                    let values: Vec<Option<bool>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(BooleanArray::from(values))
                }
                DataType::Utf8 => {
                    let mut builder = StringBuilder::new();
                    for value in column_data {
                        match value {
                            Some(v) => builder.append_value(v),
                            None => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::Date32 => {
                    let values: Vec<Option<i32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Date32Array::from(values))
                }
                DataType::Timestamp(_unit, tz) => {
                    let values: Vec<Option<i64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(TimestampMicrosecondArray::from(values).with_timezone_opt(tz.clone()))
                }
                _ => {
                    return Err(ExportError::UnsupportedDataType {
                        data_type: format!("{:?}", field.data_type()),
                    });
                }
            };

            columns.push(array);
        }

        RecordBatch::try_new(schema, columns).context(ArrowSnafu)
    }
}

#[async_trait::async_trait]
impl crate::parquet::ParquetExporter for PostgresExporter {
    async fn export_table(
        &self,
        database: &str,
        schema: Option<&str>,
        table: &str,
    ) -> Result<TableMetadata, ExportError> {
        // Create metadata
        let mut metadata = TableMetadata::new(
            DatabaseType::PostgreSQL,
            database.to_string(),
            schema.map(|s| s.to_string()),
            table.to_string(),
        );

        // Get table columns metadata
        let columns = self.get_table_columns(schema, table).await?;
        metadata.table_info.columns = columns;

        // Get Arrow schema from columns
        let arrow_schema = self.create_arrow_schema(&metadata.table_info.columns)?;
        let arrow_schema = Arc::new(arrow_schema);

        // Create output path
        let base_path = self.options.storage_config.get_base_path();
        let snapshot_path = metadata.get_snapshot_path(&base_path);

        // Create directory
        std::fs::create_dir_all(&snapshot_path).map_err(|e| ExportError::Storage {
            source: StorageError::WriteFile {
                path: snapshot_path.clone(),
                source: e,
            },
        })?;

        let parquet_path = format!("{}/data.parquet", snapshot_path);
        let metadata_path = format!("{}/metadata.json", snapshot_path);

        // Export data to Parquet
        let row_count = self
            .export_table_data(schema, table, &parquet_path, arrow_schema.clone())
            .await?;

        // Update metadata with actual row count
        metadata.table_info.row_count = row_count;

        // Save metadata
        let metadata_json =
            serde_json::to_string_pretty(&metadata).map_err(|e| ExportError::Storage {
                source: StorageError::WriteFile {
                    path: metadata_path.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                },
            })?;

        self.options
            .storage_config
            .write_bytes(&metadata_path, metadata_json.as_bytes())
            .await
            .context(StorageSnafu)?;

        Ok(metadata)
    }
}

// MySQL Exporter
pub struct MysqlExporter {
    pool: sqlx::MySqlPool,
    options: ExportOptions,
}

impl MysqlExporter {
    pub fn new(pool: sqlx::MySqlPool, options: ExportOptions) -> Self {
        Self { pool, options }
    }

    async fn get_table_columns(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<Vec<ColumnMetadata>, ExportError> {
        let database = schema.unwrap_or("mysql");

        let query = r#"
            SELECT 
                c.COLUMN_NAME as column_name,
                c.DATA_TYPE as data_type,
                c.IS_NULLABLE as is_nullable,
                c.COLUMN_DEFAULT as column_default,
                CASE 
                    WHEN c.COLUMN_KEY = 'PRI' THEN true 
                    ELSE false 
                END as is_primary_key
            FROM INFORMATION_SCHEMA.COLUMNS c
            WHERE c.TABLE_SCHEMA = ? 
                AND c.TABLE_NAME = ?
            ORDER BY c.ORDINAL_POSITION
        "#;

        let rows = sqlx::query(query)
            .bind(database)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .context(MetadataQuerySnafu)?;

        let mut columns = Vec::new();
        for row in rows {
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let is_nullable: String = row.get("is_nullable");
            let column_default: Option<String> = row.try_get("column_default").ok().flatten();
            let is_primary_key: bool = row.get("is_primary_key");

            columns.push(ColumnMetadata {
                name: column_name,
                data_type,
                is_nullable: is_nullable == "YES",
                default_value: column_default,
                is_primary_key,
            });
        }

        Ok(columns)
    }

    fn mysql_type_to_arrow(mysql_type: &str) -> Result<DataType, ExportError> {
        match mysql_type.to_lowercase().as_str() {
            "tinyint" | "smallint" => Ok(DataType::Int16),
            "mediumint" | "int" | "integer" => Ok(DataType::Int32),
            "bigint" => Ok(DataType::Int64),
            "float" => Ok(DataType::Float32),
            "double" | "real" => Ok(DataType::Float64),
            "bit" | "bool" | "boolean" => Ok(DataType::Boolean),
            "char" | "varchar" | "text" | "tinytext" | "mediumtext" | "longtext" => {
                Ok(DataType::Utf8)
            }
            "date" => Ok(DataType::Date32),
            "datetime" | "timestamp" => Ok(DataType::Timestamp(TimeUnit::Microsecond, None)),
            "decimal" | "numeric" => Ok(DataType::Decimal128(38, 10)), // Default precision/scale
            "binary" | "varbinary" | "blob" | "tinyblob" | "mediumblob" | "longblob" => {
                Ok(DataType::Binary)
            }
            "json" => Ok(DataType::Utf8), // Store JSON as string
            _ => Err(ExportError::UnsupportedDataType {
                data_type: mysql_type.to_string(),
            }),
        }
    }

    fn create_arrow_schema(&self, columns: &[ColumnMetadata]) -> Result<Schema, ExportError> {
        let fields: Result<Vec<_>, _> = columns
            .iter()
            .map(|col| {
                let data_type = Self::mysql_type_to_arrow(&col.data_type)?;
                Ok(Field::new(&col.name, data_type, col.is_nullable))
            })
            .collect();

        Ok(Schema::new(fields?))
    }

    async fn export_table_data(
        &self,
        schema: Option<&str>,
        table: &str,
        output_path: &str,
        arrow_schema: Arc<Schema>,
    ) -> Result<i64, ExportError> {
        let database = schema.unwrap_or("mysql");
        let query = format!("SELECT * FROM `{}`.`{}`", database, table);

        // Create Parquet writer
        let props = WriterProperties::builder()
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();

        let file = std::fs::File::create(output_path).map_err(|e| ExportError::Storage {
            source: StorageError::WriteFile {
                path: output_path.to_string(),
                source: e,
            },
        })?;

        let mut writer = ArrowWriter::try_new(file, arrow_schema.clone(), Some(props))
            .context(ParquetWriteSnafu)?;

        // Export data in batches
        let mut total_rows = 0i64;
        let mut rows = sqlx::query(&query).fetch(&self.pool);

        use futures::StreamExt;
        let mut batch_data: Vec<Vec<Option<String>>> = Vec::new();

        while let Some(row) = rows.next().await {
            let row = row.context(DataExportSnafu)?;

            // Convert row to string representations for simplicity
            let mut row_data = Vec::new();
            for (i, field) in arrow_schema.fields().iter().enumerate() {
                let value: Option<String> = match field.data_type() {
                    DataType::Int16 => row
                        .try_get::<Option<i16>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Int32 => row
                        .try_get::<Option<i32>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Int64 => row
                        .try_get::<Option<i64>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Float32 => row
                        .try_get::<Option<f32>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Float64 => row
                        .try_get::<Option<f64>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Boolean => row
                        .try_get::<Option<bool>, _>(i)
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    DataType::Utf8 => row.try_get::<Option<String>, _>(i).ok().flatten(),
                    DataType::Date32 => row
                        .try_get::<Option<NaiveDate>, _>(i)
                        .ok()
                        .flatten()
                        .map(|d| {
                            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                            (d.signed_duration_since(epoch).num_days()) as i32
                        })
                        .map(|v| v.to_string()),
                    DataType::Timestamp(_, _) => row
                        .try_get::<Option<NaiveDateTime>, _>(i)
                        .ok()
                        .flatten()
                        .map(|dt| dt.and_utc().timestamp_micros())
                        .map(|v| v.to_string()),
                    _ => None,
                };
                row_data.push(value);
            }

            batch_data.push(row_data);

            if batch_data.len() >= self.options.batch_size {
                let batch = self.create_record_batch(&batch_data, arrow_schema.clone())?;
                writer.write(&batch).context(ParquetWriteSnafu)?;
                total_rows += batch_data.len() as i64;
                batch_data.clear();
            }
        }

        // Write remaining data
        if !batch_data.is_empty() {
            let batch = self.create_record_batch(&batch_data, arrow_schema)?;
            writer.write(&batch).context(ParquetWriteSnafu)?;
            total_rows += batch_data.len() as i64;
        }

        writer.close().context(ParquetWriteSnafu)?;

        Ok(total_rows)
    }

    fn create_record_batch(
        &self,
        data: &[Vec<Option<String>>],
        schema: Arc<Schema>,
    ) -> Result<RecordBatch, ExportError> {
        let mut columns: Vec<ArrayRef> = Vec::new();

        for (field_idx, field) in schema.fields().iter().enumerate() {
            let column_data: Vec<Option<&str>> =
                data.iter().map(|row| row[field_idx].as_deref()).collect();

            let array: ArrayRef = match field.data_type() {
                DataType::Int16 => {
                    let values: Vec<Option<i16>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int16Array::from(values))
                }
                DataType::Int32 => {
                    let values: Vec<Option<i32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int32Array::from(values))
                }
                DataType::Int64 => {
                    let values: Vec<Option<i64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Int64Array::from(values))
                }
                DataType::Float32 => {
                    let values: Vec<Option<f32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Float32Array::from(values))
                }
                DataType::Float64 => {
                    let values: Vec<Option<f64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Float64Array::from(values))
                }
                DataType::Boolean => {
                    let values: Vec<Option<bool>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(BooleanArray::from(values))
                }
                DataType::Utf8 => {
                    let mut builder = StringBuilder::new();
                    for value in column_data {
                        match value {
                            Some(v) => builder.append_value(v),
                            None => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::Date32 => {
                    let values: Vec<Option<i32>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(Date32Array::from(values))
                }
                DataType::Timestamp(_unit, tz) => {
                    let values: Vec<Option<i64>> = column_data
                        .iter()
                        .map(|v| v.and_then(|s| s.parse().ok()))
                        .collect();
                    Arc::new(TimestampMicrosecondArray::from(values).with_timezone_opt(tz.clone()))
                }
                _ => {
                    return Err(ExportError::UnsupportedDataType {
                        data_type: format!("{:?}", field.data_type()),
                    });
                }
            };

            columns.push(array);
        }

        RecordBatch::try_new(schema, columns).context(ArrowSnafu)
    }
}

#[async_trait::async_trait]
impl crate::parquet::ParquetExporter for MysqlExporter {
    async fn export_table(
        &self,
        database: &str,
        schema: Option<&str>,
        table: &str,
    ) -> Result<TableMetadata, ExportError> {
        // Create metadata
        let mut metadata = TableMetadata::new(
            DatabaseType::MySQL,
            database.to_string(),
            schema.map(|s| s.to_string()),
            table.to_string(),
        );

        // Get table columns metadata
        let columns = self.get_table_columns(schema, table).await?;
        metadata.table_info.columns = columns;

        // Get Arrow schema from columns
        let arrow_schema = self.create_arrow_schema(&metadata.table_info.columns)?;
        let arrow_schema = Arc::new(arrow_schema);

        // Create output path
        let base_path = self.options.storage_config.get_base_path();
        let snapshot_path = metadata.get_snapshot_path(&base_path);

        // Create directory
        std::fs::create_dir_all(&snapshot_path).map_err(|e| ExportError::Storage {
            source: StorageError::WriteFile {
                path: snapshot_path.clone(),
                source: e,
            },
        })?;

        let parquet_path = format!("{}/data.parquet", snapshot_path);
        let metadata_path = format!("{}/metadata.json", snapshot_path);

        // Export data to Parquet
        let row_count = self
            .export_table_data(schema, table, &parquet_path, arrow_schema.clone())
            .await?;

        // Update metadata with actual row count
        metadata.table_info.row_count = row_count;

        // Save metadata
        let metadata_json =
            serde_json::to_string_pretty(&metadata).map_err(|e| ExportError::Storage {
                source: StorageError::WriteFile {
                    path: metadata_path.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                },
            })?;

        self.options
            .storage_config
            .write_bytes(&metadata_path, metadata_json.as_bytes())
            .await
            .context(StorageSnafu)?;

        Ok(metadata)
    }
}
