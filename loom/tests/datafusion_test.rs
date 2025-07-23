//! Integration tests for DataFusion functionality

use std::sync::Arc;

use loom::Loom;

#[tokio::test]
async fn test_query_engine_creation() {
    // This test requires a database connection
    let database_url = std::env::var("DATABASE_URL").ok();
    if database_url.is_none() {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    }

    let loom = Loom::new_from_url(&database_url.unwrap())
        .await
        .expect("Failed to create Loom instance");

    let engine = Arc::new(loom)
        .query_engine()
        .await
        .expect("Failed to create QueryEngine");

    // Verify engine was created successfully by running a simple query
    let result = engine
        .query_to_arrow("SELECT 'test' as msg")
        .await
        .expect("Failed to execute test query");

    assert_eq!(result.len(), 1);
    println!("QueryEngine created successfully");
}

#[tokio::test]
async fn test_simple_query() {
    let database_url = std::env::var("DATABASE_URL").ok();
    if database_url.is_none() {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    }

    let loom = Loom::new_from_url(&database_url.unwrap())
        .await
        .expect("Failed to create Loom instance");

    // Test a simple query without table registration
    let result = loom
        .query_to_arrow("SELECT 1 as test_column")
        .await
        .expect("Failed to execute query");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].num_rows(), 1);
    assert_eq!(result[0].num_columns(), 1);
}
