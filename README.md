# AstraCup Backend

基于Rust + Axum + SeaORM的osu!比赛管理后端系统。

## 功能特性

- ✅ osu! OAuth2认证
- ✅ JWT令牌管理
- ✅ MySQL数据库集成
- ✅ RESTful API设计
- ✅ 错误处理中间件
- ✅ CORS支持
- ✅ 结构化日志

## 技术栈

- **Web框架**: Axum 0.7
- **ORM**: SeaORM 0.12
- **数据库**: MySQL 8.0+
- **认证**: JWT + osu! OAuth2
- **序列化**: serde + serde_json
- **配置管理**: dotenv
- **日志**: tracing + tracing-subscriber
- **HTTP客户端**: reqwest

## 项目结构

```
astracup-backend/
├── src/
│   ├── main.rs              # 应用入口点
│   ├── config/              # 配置管理
│   │   ├── mod.rs
│   │   └── settings.rs
│   ├── database/            # 数据库连接
│   │   ├── mod.rs
│   │   └── connection.rs
│   ├── entities/            # SeaORM实体（待生成）
│   ├── migrations/          # 数据库迁移（待生成）
│   ├── services/            # 业务逻辑层
│   │   └── auth/           # 认证服务
│   ├── handlers/           # 请求处理器
│   │   └── auth/           # 认证处理器
│   ├── routes/             # 路由定义
│   │   ├── mod.rs
│   │   └── api.rs
│   ├── utils/              # 工具函数
│   │   ├── mod.rs
│   │   ├── error.rs        # 错误处理
│   │   └── response.rs     # 响应格式
│   └── middleware/         # 中间件（待实现）
├── .env.example            # 环境变量示例
├── Cargo.toml             # 依赖配置
└── README.md              # 项目说明
```

## 快速开始

### 1. 环境要求

- Rust 1.70+ ([安装指南](https://www.rust-lang.org/tools/install))
- MySQL 8.0+
- osu! API客户端ID和密钥

### 2. 配置环境变量

复制环境变量模板：

```bash
cp .env.example .env
```

编辑 `.env` 文件：

```env
# 数据库配置
DATABASE_URL=mysql://username:password@localhost:3306/astracup

# osu! OAuth配置
OSU_CLIENT_ID=your_client_id
OSU_CLIENT_SECRET=your_client_secret
OSU_REDIRECT_URI=http://localhost:3000/api/auth/callback/osu

# JWT配置
JWT_SECRET=your_jwt_secret_key_here_change_this_to_something_secure
JWT_EXPIRY_HOURS=24

# 服务器配置
PORT=3001
HOST=0.0.0.0
LOG_LEVEL=info

# CORS配置
CORS_ORIGIN=http://localhost:3000
```

### 3. 初始化数据库

确保MySQL服务运行，并创建数据库：

```sql
CREATE DATABASE IF NOT EXISTS astracup CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

### 4. 构建和运行

```bash
# 安装依赖
cargo build

# 运行开发服务器
cargo run

# 或使用cargo watch自动重启
cargo install cargo-watch
cargo watch -x run
```

### 5. 测试API

服务器启动后，可以测试以下端点：

```bash
# 获取osu!认证URL
curl http://localhost:3001/api/auth/url

# 验证服务器状态
curl http://localhost:3001/health
```

## API文档

### 认证相关

#### 获取osu!认证URL
```
GET /api/auth/url
```

响应：
```json
{
  "success": true,
  "auth_url": "https://osu.ppy.sh/oauth/authorize?client_id=..."
}
```

#### osu!认证回调
```
GET /api/auth/callback/osu?code={authorization_code}
```

响应：
```json
{
  "success": true,
  "token": "jwt_token_here",
  "user": {
    "id": 123456,
    "username": "PlayerName",
    "avatar_url": "https://...",
    "cover_url": "https://...",
    "country_code": "CN",
    "statistics": {
      "pp": 4500.5,
      "global_rank": 1000,
      "country_rank": 50
    }
  }
}
```

#### 登出
```
POST /api/auth/logout
```

响应：
```json
{
  "success": true,
  "message": "Logout successful. Please remove the token on the client side."
}
```

#### 验证令牌
```
GET /api/auth/verify
Authorization: Bearer {jwt_token}
```

响应：
```json
{
  "success": true,
  "valid": true,
  "user_id": "123456",
  "username": "PlayerName"
}
```

## 开发指南

### 添加新的API端点

1. 在 `services/` 目录下创建业务逻辑
2. 在 `handlers/` 目录下创建请求处理器
3. 在 `routes/api.rs` 中注册路由

### 数据库操作

使用SeaORM进行数据库操作：

```rust
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

// 查询示例
let users = User::find()
    .filter(UserColumn::Username.contains("test"))
    .all(&db.connection)
    .await?;
```

### 错误处理

使用 `AppError` 枚举处理错误：

```rust
use crate::utils::AppError;

async fn some_handler() -> Result<Json<Response>, AppError> {
    // 业务逻辑
    if something_wrong {
        return Err(AppError::NotFound("Resource not found".to_string()));
    }
    Ok(Json(response))
}
```

## 部署

### 生产环境构建

```bash
# 发布构建
cargo build --release

# 运行生产服务器
./target/release/astracup-backend
```

### Docker部署

```dockerfile
FROM rust:1.70 as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/astracup-backend /usr/local/bin/
COPY .env.production .env
EXPOSE 3001
CMD ["astracup-backend"]
```

## 迁移计划

本项目是从现有的Next.js API后端迁移而来。完整的迁移计划包括：

1. ✅ 项目初始化和基础架构
2. ⏳ 数据库模型和迁移
3. ⏳ 核心业务逻辑实现
4. ⏳ 剩余API端点迁移
5. ⏳ 测试和优化
6. ⏳ 部署和文档

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

## 许可证

MIT License - 详见 LICENSE 文件