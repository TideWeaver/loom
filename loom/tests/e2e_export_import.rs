//! End-to-end tests for export/import functionality

use loom::parquet::{ExportOptions, ImportOptions};
use loom::Loom;
use sqlx::{Executor, PgPool, MySqlPool};
use std::env;

#[tokio::test]
async fn test_pg_basic_table_export_import() -> Result<(), Box<dyn std::error::Error>> {
    // Skip test if no database URL is provided
    let database_url = env::var("PG_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));
    if database_url.is_err() {
        eprintln!("Skipping PostgreSQL test - no PG_DATABASE_URL or DATABASE_URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "postgres").await?;
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    
    // Create test table with various data types
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_basic;
        CREATE TABLE test_basic (
            id INTEGER PRIMARY KEY,
            name VARCHAR(100),
            active BOOLEAN,
            score FLOAT,
            created_date DATE,
            updated_at TIMESTAMP
        );
        "#,
    )
    .await?;
    
    // Insert test data
    pool.execute(
        r#"
        INSERT INTO test_basic (id, name, active, score, created_date, updated_at) VALUES
        (1, 'Alice', true, 95.5, '2024-01-01', '2024-01-01 10:00:00'),
        (2, 'Bob', false, 87.3, '2024-01-02', '2024-01-02 11:30:00'),
        (3, 'Charlie', true, 92.1, '2024-01-03', '2024-01-03 15:45:00'),
        (4, NULL, NULL, NULL, NULL, NULL);
        "#,
    )
    .await?;
    
    // Export the table
    let export_metadata = loom.export_table("postgres", Some("public"), "test_basic").await?;
    assert_eq!(export_metadata.table_info.row_count, 4);
    assert_eq!(export_metadata.table_info.columns.len(), 6);
    
    // Drop the table
    pool.execute("DROP TABLE test_basic").await?;
    
    // Import the data back
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        truncate_before_import: false,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 4);
    
    // Verify the data
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_basic")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 4);
    
    // Verify specific values
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM test_basic WHERE id = 1")
        .fetch_one(&pool)
        .await?;
    assert_eq!(name, Some("Alice".to_string()));
    
    // Cleanup
    pool.execute("DROP TABLE test_basic").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_pg_complex_types() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("PG_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));
    if database_url.is_err() {
        eprintln!("Skipping PostgreSQL complex types test - no database URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "postgres").await?;
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    
    // Create table with more complex types
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_complex;
        CREATE TABLE test_complex (
            id BIGINT PRIMARY KEY,
            small_num SMALLINT,
            big_decimal DECIMAL(10,2),
            json_data JSONB,
            uuid_col UUID,
            byte_data BYTEA,
            ts_with_tz TIMESTAMP WITH TIME ZONE
        );
        "#,
    )
    .await?;
    
    // Insert test data
    pool.execute(
        r#"
        INSERT INTO test_complex VALUES
        (1, 100, 1234.56, '{"key": "value"}', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', E'\\xDEADBEEF', '2024-01-01 10:00:00+00'),
        (2, -32768, 9999999.99, '["array", "values"]', '550e8400-e29b-41d4-a716-446655440000', E'\\x00FF00FF', '2024-12-31 23:59:59+00');
        "#,
    )
    .await?;
    
    // Export
    let export_metadata = loom.export_table("postgres", Some("public"), "test_complex").await?;
    assert_eq!(export_metadata.table_info.row_count, 2);
    
    // Drop and reimport
    pool.execute("DROP TABLE test_complex").await?;
    
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 2);
    
    // Verify count
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_complex")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 2);
    
    // Cleanup
    pool.execute("DROP TABLE test_complex").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_mysql_basic_table_export_import() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MYSQL_DATABASE_URL");
    if database_url.is_err() {
        eprintln!("Skipping MySQL test - no MYSQL_DATABASE_URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "mysql").await?;
    let pool = sqlx::mysql::MySqlPool::connect(&database_url).await?;
    
    // Create test table
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_basic;
        CREATE TABLE test_basic (
            id INT PRIMARY KEY,
            name VARCHAR(100),
            active BOOLEAN,
            score FLOAT,
            created_date DATE,
            updated_at DATETIME
        );
        "#,
    )
    .await?;
    
    // Insert test data
    pool.execute(
        r#"
        INSERT INTO test_basic (id, name, active, score, created_date, updated_at) VALUES
        (1, 'Alice', true, 95.5, '2024-01-01', '2024-01-01 10:00:00'),
        (2, 'Bob', false, 87.3, '2024-01-02', '2024-01-02 11:30:00'),
        (3, 'Charlie', true, 92.1, '2024-01-03', '2024-01-03 15:45:00'),
        (4, NULL, NULL, NULL, NULL, NULL);
        "#,
    )
    .await?;
    
    // Get the database name from the URL
    let db_name = database_url.split('/').last().unwrap_or("test");
    
    // Export the table
    let export_metadata = loom.export_table(db_name, None, "test_basic").await?;
    assert_eq!(export_metadata.table_info.row_count, 4);
    
    // Drop the table
    pool.execute("DROP TABLE test_basic").await?;
    
    // Import the data back
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 4);
    
    // Verify the data
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_basic")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 4);
    
    // Cleanup
    pool.execute("DROP TABLE test_basic").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_mysql_complex_types() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MYSQL_DATABASE_URL");
    if database_url.is_err() {
        eprintln!("Skipping MySQL complex types test - no MYSQL_DATABASE_URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "mysql").await?;
    let pool = sqlx::mysql::MySqlPool::connect(&database_url).await?;
    
    // Create table with various MySQL types
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_complex;
        CREATE TABLE test_complex (
            id BIGINT PRIMARY KEY,
            tiny_num TINYINT,
            medium_num MEDIUMINT,
            big_decimal DECIMAL(10,2),
            double_num DOUBLE,
            bit_col BIT(1),
            text_col TEXT,
            blob_col BLOB,
            json_col JSON,
            ts_col TIMESTAMP
        );
        "#,
    )
    .await?;
    
    // Insert test data
    pool.execute(
        r#"
        INSERT INTO test_complex VALUES
        (1, 127, 8388607, 1234.56, 3.14159, 1, 'Long text content', X'DEADBEEF', '{"key": "value"}', '2024-01-01 10:00:00'),
        (2, -128, -8388608, 9999999.99, 2.71828, 0, 'Another text', X'00FF00FF', '["array"]', '2024-12-31 23:59:59');
        "#,
    )
    .await?;
    
    let db_name = database_url.split('/').last().unwrap_or("test");
    
    // Export
    let export_metadata = loom.export_table(db_name, None, "test_complex").await?;
    assert_eq!(export_metadata.table_info.row_count, 2);
    
    // Drop and reimport
    pool.execute("DROP TABLE test_complex").await?;
    
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 2);
    
    // Verify count
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_complex")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 2);
    
    // Cleanup
    pool.execute("DROP TABLE test_complex").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_large_table_export_import() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("PG_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));
    if database_url.is_err() {
        eprintln!("Skipping large table test - no database URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "postgres").await?;
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    
    // Create table
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_large;
        CREATE TABLE test_large (
            id INTEGER PRIMARY KEY,
            data TEXT,
            value FLOAT
        );
        "#,
    )
    .await?;
    
    // Insert 10000 rows
    pool.execute(
        r#"
        INSERT INTO test_large (id, data, value)
        SELECT 
            generate_series(1, 10000),
            'Data for row ' || generate_series(1, 10000),
            random() * 1000
        ;
        "#,
    )
    .await?;
    
    // Export
    let export_metadata = loom.export_table("postgres", Some("public"), "test_large").await?;
    assert_eq!(export_metadata.table_info.row_count, 10000);
    
    // Drop and reimport
    pool.execute("DROP TABLE test_large").await?;
    
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 10000);
    
    // Verify
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_large")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 10000);
    
    // Cleanup
    pool.execute("DROP TABLE test_large").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_empty_table() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("PG_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));
    if database_url.is_err() {
        eprintln!("Skipping empty table test - no database URL set");
        return Ok(());
    }
    
    let database_url = database_url.unwrap();
    let loom = Loom::new_from_url(&database_url, "postgres").await?;
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    
    // Create empty table
    pool.execute(
        r#"
        DROP TABLE IF EXISTS test_empty;
        CREATE TABLE test_empty (
            id INTEGER PRIMARY KEY,
            name VARCHAR(100)
        );
        "#,
    )
    .await?;
    
    // Export empty table
    let export_metadata = loom.export_table("postgres", Some("public"), "test_empty").await?;
    assert_eq!(export_metadata.table_info.row_count, 0);
    assert_eq!(export_metadata.table_info.columns.len(), 2);
    
    // Drop and reimport
    pool.execute("DROP TABLE test_empty").await?;
    
    let import_options = ImportOptions {
        create_table_if_not_exists: true,
        ..Default::default()
    };
    
    let snapshot_path = export_metadata.get_snapshot_path("");
    let imported_rows = loom.import_snapshot(&snapshot_path, import_options).await?;
    assert_eq!(imported_rows, 0);
    
    // Verify table exists but is empty
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_empty")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    
    // Cleanup
    pool.execute("DROP TABLE test_empty").await?;
    
    Ok(())
}