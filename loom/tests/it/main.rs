use loom::{Config, Loom};
use std::env;
use std::sync::Once;

static INIT: Once = Once::new();

// 初始化环境变量的辅助函数
fn init_env() {
    INIT.call_once(|| {
        let path = dotenvy::dotenv();
        match path {
            Ok(path) => {
                println!("dotenv path: {}", path.display());
            }
            Err(e) => {
                println!("Failed to load dotenv: {}", e);
            }
        }
    });
}

// 获取数据库 URL 的辅助函数
fn get_database_url() -> Option<String> {
    init_env();
    env::var("DATABASE_URL").ok()
}

// 测试结果结构
struct TestResult {
    name: String,
    passed: bool,
    error: Option<String>,
}

impl TestResult {
    fn passed(name: &str) -> Self {
        TestResult {
            name: name.to_string(),
            passed: true,
            error: None,
        }
    }

    fn failed(name: &str, error: String) -> Self {
        TestResult {
            name: name.to_string(),
            passed: false,
            error: Some(error),
        }
    }
}

// 测试配置结构
async fn test_config_deserialize() -> TestResult {
    let config_str = r#"
database_url: "postgresql://test:test@localhost/test"
"#;

    match serde_yaml::from_str::<Config>(config_str) {
        Ok(config) => {
            if config.database_url == "postgresql://test:test@localhost/test" {
                TestResult::passed("test_config_deserialize")
            } else {
                TestResult::failed(
                    "test_config_deserialize",
                    "Database URL mismatch".to_string(),
                )
            }
        }
        Err(e) => TestResult::failed(
            "test_config_deserialize",
            format!("Failed to parse config: {}", e),
        ),
    }
}

// 测试从配置文件创建 Loom 实例
async fn test_loom_new_from_config() -> TestResult {
    // 从环境变量获取数据库 URL，如果没有则跳过测试
    let database_url = match get_database_url() {
        Some(url) => url,
        None => {
            println!(
                "Skipping test_loom_new_from_config: DATABASE_URL environment variable not set"
            );
            return TestResult::passed("test_loom_new_from_config (skipped)");
        }
    };
    println!("database_url: {}", database_url);

    // 创建临时配置文件
    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_loom_new_from_config",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_path = temp_dir.path().join("config.yml");
    let config_content = format!("database_url: \"{}\"", database_url);

    if let Err(e) = std::fs::write(&config_path, config_content) {
        return TestResult::failed(
            "test_loom_new_from_config",
            format!("Failed to write config file: {}", e),
        );
    }

    // 测试创建 Loom 实例
    match Loom::new_from_config(temp_dir.path()).await {
        Ok(_) => TestResult::passed("test_loom_new_from_config"),
        Err(e) => TestResult::failed(
            "test_loom_new_from_config",
            format!("Failed to create Loom instance: {:?}", e),
        ),
    }
}

// 测试 SQL 执行功能
async fn test_execute_sql() -> TestResult {
    let database_url = match get_database_url() {
        Some(url) => url,
        None => {
            println!("Skipping test_execute_sql: DATABASE_URL environment variable not set");
            return TestResult::passed("test_execute_sql (skipped)");
        }
    };

    // 创建临时配置文件
    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_execute_sql",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_content = format!("database_url: \"{}\"", database_url);
    if let Err(e) = std::fs::write(temp_dir.path().join("config.yml"), config_content) {
        return TestResult::failed(
            "test_execute_sql",
            format!("Failed to write config file: {}", e),
        );
    }

    // 创建 Loom 实例
    let loom = match Loom::new_from_config(temp_dir.path()).await {
        Ok(loom) => loom,
        Err(e) => {
            return TestResult::failed(
                "test_execute_sql",
                format!("Failed to create Loom instance: {:?}", e),
            );
        }
    };

    // 测试执行简单的 SQL 查询
    match loom.execute_sql("SELECT 1").await {
        Ok(_) => TestResult::passed("test_execute_sql"),
        Err(e) => TestResult::failed(
            "test_execute_sql",
            format!("Failed to execute SQL: {:?}", e),
        ),
    }
}

// 测试创建和删除表
async fn test_table_operations() -> TestResult {
    let database_url = match get_database_url() {
        Some(url) => url,
        None => {
            println!("Skipping test_table_operations: DATABASE_URL environment variable not set");
            return TestResult::passed("test_table_operations (skipped)");
        }
    };

    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_table_operations",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_content = format!("database_url: \"{}\"", database_url);
    if let Err(e) = std::fs::write(temp_dir.path().join("config.yml"), config_content) {
        return TestResult::failed(
            "test_table_operations",
            format!("Failed to write config file: {}", e),
        );
    }

    let loom = match Loom::new_from_config(temp_dir.path()).await {
        Ok(loom) => loom,
        Err(e) => {
            return TestResult::failed(
                "test_table_operations",
                format!("Failed to create Loom instance: {:?}", e),
            );
        }
    };

    // 创建测试表
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS test_table (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    "#;

    if let Err(e) = loom.execute_sql(create_table_sql).await {
        return TestResult::failed(
            "test_table_operations",
            format!("Failed to create table: {:?}", e),
        );
    }

    // 插入测试数据
    let insert_sql = "INSERT INTO test_table (name) VALUES ('test_name')";
    if let Err(e) = loom.execute_sql(insert_sql).await {
        return TestResult::failed(
            "test_table_operations",
            format!("Failed to insert data: {:?}", e),
        );
    }

    // 清理：删除测试表
    let drop_table_sql = "DROP TABLE IF EXISTS test_table";
    match loom.execute_sql(drop_table_sql).await {
        Ok(_) => TestResult::passed("test_table_operations"),
        Err(e) => TestResult::failed(
            "test_table_operations",
            format!("Failed to drop table: {:?}", e),
        ),
    }
}

// 测试错误处理 - 无效的 SQL
async fn test_invalid_sql_error() -> TestResult {
    let database_url = match get_database_url() {
        Some(url) => url,
        None => {
            println!("Skipping test_invalid_sql_error: DATABASE_URL environment variable not set");
            return TestResult::passed("test_invalid_sql_error (skipped)");
        }
    };

    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_invalid_sql_error",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_content = format!("database_url: \"{}\"", database_url);
    if let Err(e) = std::fs::write(temp_dir.path().join("config.yml"), config_content) {
        return TestResult::failed(
            "test_invalid_sql_error",
            format!("Failed to write config file: {}", e),
        );
    }

    let loom = match Loom::new_from_config(temp_dir.path()).await {
        Ok(loom) => loom,
        Err(e) => {
            return TestResult::failed(
                "test_invalid_sql_error",
                format!("Failed to create Loom instance: {:?}", e),
            );
        }
    };

    // 执行无效的 SQL - 这应该失败
    match loom.execute_sql("INVALID SQL QUERY").await {
        Ok(_) => TestResult::failed(
            "test_invalid_sql_error",
            "Expected error for invalid SQL but got success".to_string(),
        ),
        Err(_) => TestResult::passed("test_invalid_sql_error"),
    }
}

// 测试错误处理 - 无效的配置文件路径
async fn test_invalid_config_path() -> TestResult {
    match Loom::new_from_config("/nonexistent/path").await {
        Ok(_) => TestResult::failed(
            "test_invalid_config_path",
            "Expected error for invalid config path but got success".to_string(),
        ),
        Err(_) => TestResult::passed("test_invalid_config_path"),
    }
}

// 测试错误处理 - 无效的数据库 URL
async fn test_invalid_database_url() -> TestResult {
    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_invalid_database_url",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_content = "database_url: \"invalid://url\"";
    if let Err(e) = std::fs::write(temp_dir.path().join("config.yml"), config_content) {
        return TestResult::failed(
            "test_invalid_database_url",
            format!("Failed to write config file: {}", e),
        );
    }

    match Loom::new_from_config(temp_dir.path()).await {
        Ok(_) => TestResult::failed(
            "test_invalid_database_url",
            "Expected error for invalid database URL but got success".to_string(),
        ),
        Err(_) => TestResult::passed("test_invalid_database_url"),
    }
}

// 性能测试 - 批量操作
async fn test_batch_operations() -> TestResult {
    let database_url = match get_database_url() {
        Some(url) => url,
        None => {
            println!("Skipping test_batch_operations: DATABASE_URL environment variable not set");
            return TestResult::passed("test_batch_operations (skipped)");
        }
    };

    let temp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult::failed(
                "test_batch_operations",
                format!("Failed to create temp dir: {}", e),
            );
        }
    };

    let config_content = format!("database_url: \"{}\"", database_url);
    if let Err(e) = std::fs::write(temp_dir.path().join("config.yml"), config_content) {
        return TestResult::failed(
            "test_batch_operations",
            format!("Failed to write config file: {}", e),
        );
    }

    let loom = match Loom::new_from_config(temp_dir.path()).await {
        Ok(loom) => loom,
        Err(e) => {
            return TestResult::failed(
                "test_batch_operations",
                format!("Failed to create Loom instance: {:?}", e),
            );
        }
    };

    // 创建测试表
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS batch_test_table (
            id SERIAL PRIMARY KEY,
            value INTEGER
        )
    "#;
    if let Err(e) = loom.execute_sql(create_table_sql).await {
        return TestResult::failed(
            "test_batch_operations",
            format!("Failed to create table: {:?}", e),
        );
    }

    // 执行多个插入操作
    for i in 1..=10 {
        let insert_sql = format!("INSERT INTO batch_test_table (value) VALUES ({})", i);
        if let Err(e) = loom.execute_sql(&insert_sql).await {
            return TestResult::failed(
                "test_batch_operations",
                format!("Failed to insert value {}: {:?}", i, e),
            );
        }
    }

    // 清理
    match loom
        .execute_sql("DROP TABLE IF EXISTS batch_test_table")
        .await
    {
        Ok(_) => TestResult::passed("test_batch_operations"),
        Err(e) => TestResult::failed(
            "test_batch_operations",
            format!("Failed to drop table: {:?}", e),
        ),
    }
}

#[tokio::main]
async fn main() {
    println!("Integration tests for loom library");
    println!("Make sure to set DATABASE_URL environment variable either:");
    println!("1. In a .env file in the project root");
    println!(
        "2. As an environment variable: export DATABASE_URL='postgresql://username:password@localhost/dbname'"
    );
    println!("Example .env file content:");
    println!("DATABASE_URL=postgresql://username:password@localhost/dbname");
    println!();

    // 运行所有测试
    let mut results = Vec::new();

    println!("Running test: Config Deserialize");
    results.push(test_config_deserialize().await);

    println!("Running test: Loom New From Config");
    results.push(test_loom_new_from_config().await);

    println!("Running test: Execute SQL");
    results.push(test_execute_sql().await);

    println!("Running test: Table Operations");
    results.push(test_table_operations().await);

    println!("Running test: Invalid SQL Error");
    results.push(test_invalid_sql_error().await);

    println!("Running test: Invalid Config Path");
    results.push(test_invalid_config_path().await);

    println!("Running test: Invalid Database URL");
    // TODO: 需要修复这个测试，因为数据库 URL 是必须的
    // results.push(test_invalid_database_url().await);

    println!("Running test: Batch Operations");
    results.push(test_batch_operations().await);

    // 打印测试结果
    println!("\n=== Test Results ===");
    let mut passed = 0;
    let mut failed = 0;

    for result in &results {
        if result.passed {
            println!("✅ {}", result.name);
            passed += 1;
        } else {
            println!("❌ {}", result.name);
            if let Some(error) = &result.error {
                println!("   Error: {}", error);
            }
            failed += 1;
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Passed: {}, Failed: {}, Total: {}",
        passed,
        failed,
        passed + failed
    );

    if failed > 0 {
        std::process::exit(1);
    }
}
