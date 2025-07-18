# Loom

Loom 是一个基于 Rust 的数据库交互工具，集成了 DataFusion 查询引擎，提供高性能的数据查询和处理能力。

## 特性

- 🚀 PostgreSQL 连接池支持
- 📊 集成 Apache Arrow DataFusion 查询引擎
- 🔧 灵活的配置方式（YAML 文件或环境变量）
- 📁 多种输出格式（表格、CSV）
- ⚡ 异步执行，高性能

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/yourusername/loom.git
cd loom

# 构建项目
cargo build --release

# 运行测试
cargo test
```

## 配置

Loom 支持两种配置方式：

### 1. 配置文件（推荐）

在项目根目录创建 `config.yml` 或 `config.yaml`：

```yaml
database_url: "postgresql://username:password@localhost:5432/dbname"
```

### 2. 环境变量

```bash
# 使用 LOOM_ 前缀
export LOOM_DATABASE_URL="postgresql://username:password@localhost:5432/dbname"

# 或者在 .env 文件中设置（用于开发）
echo 'DATABASE_URL="postgresql://username:password@localhost:5432/dbname"' > .env
```

### 配置优先级

1. 环境变量（LOOM_ 前缀）
2. 配置文件（config.yml/config.yaml）
3. 默认值

## 使用方法

### CLI 命令

```bash
# 测试数据库连接
loom ping

# 执行 SQL 查询
loom exec "SELECT * FROM users LIMIT 10"

# 使用 DataFusion 查询（默认表格输出）
loom query "SELECT 1 as num, 'hello' as text"

# 输出为 CSV 格式
loom query "SELECT * FROM generate_series(1,10)" --format csv

# 保存查询结果到文件
loom query "SELECT * FROM data" --format csv --output results.csv

# 指定配置文件路径
loom --config /path/to/config.yml ping
```

### 作为库使用

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
loom = "0.0.1"
tokio = { version = "1", features = ["full"] }
```

示例代码：

```rust
use loom::Loom;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 从配置文件初始化
    let loom = Loom::new_from_config(".").await?;
    
    // 执行 SQL 查询
    loom.execute_sql("SELECT 1").await?;
    
    // 使用 DataFusion 查询引擎
    let engine = Arc::new(loom).query_engine().await?;
    let batches = engine.query_to_arrow("SELECT 1 as num, 2 as value").await?;
    
    // 处理查询结果
    for batch in batches {
        println!("行数: {}", batch.num_rows());
    }
    
    Ok(())
}
```

## DataFusion 集成

Loom 集成了 Apache Arrow DataFusion，提供强大的查询能力：

### 支持的功能

- SQL 查询执行
- Arrow 格式数据处理
- 内置函数支持（数学、字符串、日期等）
- 生成序列等特殊功能

### 示例查询

```sql
-- 生成序列
SELECT * FROM generate_series(1, 10);

-- 数学计算
SELECT sin(1.0), cos(1.0), sqrt(16);

-- 字符串处理
SELECT concat('Hello', ' ', 'World');

-- 日期处理
SELECT now(), date_part('year', now());
```

### 当前限制

- PostgreSQL 表扫描功能尚未实现
- 如需查询实际的 PostgreSQL 表，请使用 `exec` 命令

## 开发指南

### 项目结构

```
loom/
├── loom/               # 核心库
│   ├── src/
│   │   ├── lib.rs     # 库入口
│   │   ├── config.rs  # 配置管理
│   │   ├── error.rs   # 错误处理
│   │   └── datafusion/
│   │       ├── mod.rs      # DataFusion 模块
│   │       ├── engine.rs   # 查询引擎
│   │       └── provider.rs # 表提供者（待实现）
│   └── tests/         # 集成测试
├── loom-cli/          # CLI 工具
│   └── src/
│       └── main.rs    # CLI 入口
├── examples/          # 示例代码
└── Cargo.toml        # 工作空间配置
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行集成测试（需要设置 DATABASE_URL）
DATABASE_URL="postgresql://..." cargo test --test it

# 运行特定测试
cargo test config_from_file
```

### 代码格式化

```bash
# 格式化 Rust 代码
cargo fmt

# 检查代码规范
cargo clippy

# 格式化 TOML 文件
taplo fmt
```

### 示例程序

运行 DataFusion 查询示例：

```bash
cargo run --example datafusion_query
```

## 环境要求

- Rust 1.75+
- PostgreSQL 12+
- Tokio 运行时

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 致谢

- [Apache Arrow](https://arrow.apache.org/) - 列式内存格式
- [DataFusion](https://arrow.apache.org/datafusion/) - SQL 查询引擎
- [SQLx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包