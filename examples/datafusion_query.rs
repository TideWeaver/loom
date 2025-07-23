//! Example showing how to use DataFusion with Loom

use std::sync::Arc;

use loom::Loom;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Loom instance from environment variables
    println!("Creating Loom instance...");
    let loom = match std::env::var("DATABASE_URL") {
        Ok(url) => Loom::new_from_url(&url).await?,
        Err(_) => {
            eprintln!("DATABASE_URL environment variable not set");
            eprintln!("Set it with: export DATABASE_URL=postgresql://user:pass@localhost/db");
            return Ok(());
        }
    };

    // Create QueryEngine
    println!("Creating QueryEngine...");
    let engine = Arc::new(loom).query_engine().await?;

    // Example 1: Simple SELECT query
    println!("\n=== Example 1: Simple SELECT ===");
    let result = engine
        .query_to_arrow("SELECT 1 as num, 'Hello' as greeting")
        .await?;

    use datafusion::arrow::util::pretty::pretty_format_batches;
    let formatted = pretty_format_batches(&result)?;
    println!("{}", formatted);

    // Example 2: Query with calculations
    println!("\n=== Example 2: Calculations ===");
    let result = engine
        .query_to_arrow(
            "SELECT 
        10 + 5 as addition,
        20 - 3 as subtraction,
        4 * 7 as multiplication,
        100.0 / 3.0 as division
    ",
        )
        .await?;

    let formatted = pretty_format_batches(&result)?;
    println!("{}", formatted);

    // Example 3: Using VALUES clause
    println!("\n=== Example 3: VALUES clause ===");
    let result = engine
        .query_to_arrow(
            "
        SELECT * FROM (
            VALUES 
                (1, 'Alice', 30),
                (2, 'Bob', 25),
                (3, 'Charlie', 35)
        ) as users(id, name, age)
        WHERE age > 26
        ORDER BY age DESC
    ",
        )
        .await?;

    let formatted = pretty_format_batches(&result)?;
    println!("{}", formatted);

    // Example 4: Get DataFrame for further processing
    println!("\n=== Example 4: DataFrame processing ===");
    let df = engine
        .query_to_dataframe("SELECT 1 as x, 2 as y UNION ALL SELECT 3, 4 UNION ALL SELECT 5, 6")
        .await?;

    // Add a computed column
    let df_with_sum = df.with_column(
        "sum",
        datafusion::prelude::col("x") + datafusion::prelude::col("y"),
    )?;

    // Collect results
    let results = df_with_sum.collect().await?;
    let formatted = pretty_format_batches(&results)?;
    println!("{}", formatted);

    println!("\n✅ All examples completed successfully!");

    Ok(())
}
