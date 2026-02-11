# AstraCup Web 后端迁移计划

## 项目概述
将现有的Next.js API后端迁移到Rust + Axum + SeaORM架构。

## 现有API分析

### 主要功能模块
1. **认证模块** - osu! OAuth2认证
2. **报名管理** - 玩家报名、审核、管理
3. **比赛房间** - 比赛房间创建、管理
4. **比赛预约** - 比赛时间安排
5. **玩家对战** - 对战列表管理
6. **消息通知** - 系统消息通知
7. **比赛设置** - 锦标赛配置
8. **分数管理** - 比赛分数保存
9.  **文件处理** - 谱面下载、回放处理

### 数据库表结构
基于 `src/lib/mysql-registrations.ts` 分析：

1. **registrations** - 玩家报名表
2. **match_rooms** - 比赛房间表
3. **match_schedules** - 比赛预约表
4. **player_matchups** - 玩家对战表
5. **messages** - 消息通知表
6. **staff_room_assignments** - Staff分配表
7. **tournament_settings** - 比赛设置表
8. **match_scores** - 比赛分数表

## Rust后端架构设计

### 技术栈
- **Web框架**: Axum
- **ORM**: SeaORM
- **数据库**: MySQL (与现有兼容)
- **认证**: JWT + osu! OAuth2
- **配置管理**: dotenv + config
- **日志**: tracing
- **序列化**: serde
- **验证**: validator

### 项目结构
```
astracup-backend/
├── Cargo.toml
├── .env.example
├── .gitignore
├── README.md
├── src/
│   ├── main.rs
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs
│   ├── database/
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   └── models/
│   │       ├── mod.rs
│   │       ├── registration.rs
│   │       ├── match_room.rs
│   │       ├── match_schedule.rs
│   │       ├── player_matchup.rs
│   │       ├── message.rs
│   │       ├── staff_assignment.rs
│   │       ├── tournament_setting.rs
│   │       └── match_score.rs
│   ├── entities/
│   │   ├── mod.rs
│   │   └── *.rs (SeaORM生成的实体)
│   ├── migrations/
│   │   ├── mod.rs
│   │   └── *.rs (数据库迁移文件)
│   ├── services/
│   │   ├── mod.rs
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── service.rs
│   │   │   └── middleware.rs
│   │   ├── registration/
│   │   ├── match_room/
│   │   ├── match_schedule/
│   │   ├── player_matchup/
│   │   ├── message/
│   │   ├── staff_assignment/
│   │   ├── tournament_setting/
│   │   └── match_score/
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── auth/
│   │   ├── registration/
│   │   ├── match_room/
│   │   ├── match_schedule/
│   │   ├── player_matchup/
│   │   ├── message/
│   │   ├── staff_assignment/
│   │   ├── tournament_setting/
│   │   └── match_score/
│   ├── routes/
│   │   ├── mod.rs
│   │   └── api.rs
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── error.rs
│   │   ├── response.rs
│   │   └── validation.rs
│   └── middleware/
│       ├── mod.rs
│       ├── auth.rs
│       └── logging.rs
└── tests/
    ├── integration/
    └── unit/
```

### API端点映射

#### 认证相关
- `GET /api/auth/url` → `GET /api/auth/url`
- `GET /api/auth/callback/osu` → `GET /api/auth/callback/osu`
- `POST /api/auth/logout` → `POST /api/auth/logout`

#### 报名管理
- `GET /api/register` → `GET /api/registrations`
- `POST /api/register` → `POST /api/registrations`
- `GET /api/approved-players` → `GET /api/registrations/approved`
- `POST /api/admin/approve-registration` → `POST /api/admin/registrations/{id}/approve`
- `POST /api/admin/delete-registration` → `DELETE /api/admin/registrations/{id}`

#### 比赛房间
- `GET /api/match-rooms` → `GET /api/match-rooms`
- `POST /api/match-rooms/create` → `POST /api/match-rooms`
- `DELETE /api/match-rooms/delete` → `DELETE /api/match-rooms/{id}`

#### 比赛预约
- `GET /api/match-schedules` → `GET /api/match-schedules`
- `POST /api/match-schedules/create` → `POST /api/match-schedules`
- `PUT /api/match-schedules/update` → `PUT /api/match-schedules/{id}`
- `PUT /api/match-schedules/update-details` → `PATCH /api/match-schedules/{id}/details`

#### 玩家对战
- `GET /api/player-matchups` → `GET /api/player-matchups`
- `POST /api/player-matchups/create` → `POST /api/player-matchups`
- `DELETE /api/player-matchups/delete` → `DELETE /api/player-matchups/{id}`

#### 消息通知
- `GET /api/messages` → `GET /api/messages`
- `POST /api/messages/send` → `POST /api/messages`

#### Staff分配
- `GET /api/staff-room-assignments` → `GET /api/staff-assignments`
- `GET /api/available-rooms-for-staff` → `GET /api/staff-assignments/available-rooms`
- `POST /api/staff-room-assignments` → `POST /api/staff-assignments`
- `PUT /api/staff-room-assignments/{id}` → `PUT /api/staff-assignments/{id}`

#### 比赛设置
- `GET /api/tournament-settings` → `GET /api/tournament-settings`
- `PUT /api/tournament-settings` → `PUT /api/tournament-settings`

#### 分数管理
- `POST /api/match-scores/save` → `POST /api/match-scores`
- `PUT /api/match-scores/update` → `PUT /api/match-scores/{id}`
- `GET /api/multiplayer-scores` → `GET /api/match-scores`

## 迁移步骤

### 第一阶段：项目初始化 (已完成)
1. ✅ 创建Rust项目结构
2. ✅ 配置Cargo.toml依赖
3. ✅ 设置数据库连接和SeaORM
4. ✅ 创建环境配置

### 第二阶段：数据库模型和迁移 (已完成)
1. ✅ 根据现有表结构创建SeaORM实体
2. ✅ 编写数据库迁移脚本
3. ✅ 创建模型定义和关系
4. ✅ 测试数据库连接和操作

### 第三阶段：核心服务层 (已完成)
1. ✅ 实现认证服务（osu! OAuth2集成）
2. ✅ 实现报名管理服务
3. ✅ 实现比赛房间服务
4. ✅ 实现消息通知服务
5. ✅ 实现比赛设置服务

### 第四阶段：API路由和处理器 (已完成)
1. ✅ 实现Axum路由配置
2. ✅ 创建请求/响应DTO
3. ✅ 实现错误处理中间件
4. ✅ 实现认证中间件

### 第五阶段：剩余功能模块 (进行中)
1. ⏳ 比赛预约服务
2. ⏳ 玩家对战管理
3. ⏳ 分数管理系统
4. ⏳ 文件处理模块

### 第六阶段：测试和优化 (待开始)
1. ⏳ 编写单元测试
2. ⏳ 编写集成测试
3. ⏳ 性能优化
4. ⏳ 错误处理完善

### 第七阶段：部署和文档 (待开始)
1. ⏳ 创建部署配置
2. ⏳ 编写部署文档
3. ⏳ 创建API文档
4. ⏳ 迁移指南

## 当前完成状态

### 已完成的模块
1. ✅ **认证模块** - osu! OAuth2认证、JWT令牌管理
2. ✅ **报名管理** - 玩家报名、审核、管理
3. ✅ **比赛房间** - 比赛房间创建、管理
4. ✅ **消息通知** - 系统消息通知
5. ✅ **比赛设置** - 锦标赛配置
6. ✅ **用户管理** - 用户信息管理
7. ✅ **团队管理** - 团队信息管理

### 待完成的模块
1. ⏳ **比赛预约** - 比赛时间安排
2. ⏳ **玩家对战** - 对战列表管理
3. ⏳ **分数管理** - 比赛分数保存
4. ⏳ **文件处理** - 谱面下载、回放处理

### API端点完成情况
- ✅ 认证相关API (4个端点)
- ✅ 用户管理API (4个端点)
- ✅ 报名管理API (5个端点)
- ✅ 比赛房间API (6个端点)
- ✅ 消息通知API (7个端点)
- ✅ 比赛设置API (7个端点)
- ✅ 团队管理API (3个端点)
- ⏳ 比赛预约API (待实现)
- ⏳ 玩家对战API (待实现)
- ⏳ 分数管理API (待实现)

**总计：已完成 36个API端点，待完成 约15个API端点**

## 技术细节

### 数据库连接池
使用SeaORM的连接池，配置与现有MySQL兼容：
```rust
let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
let pool = Database::connect(&database_url).await?;
```

### 认证流程
1. 前端调用 `/api/auth/url` 获取osu! OAuth2授权URL
2. 用户授权后重定向到 `/api/auth/callback/osu`
3. 后端使用授权码交换访问令牌
4. 获取用户信息并创建JWT
5. 返回JWT给前端用于后续请求

### 错误处理
统一错误响应格式：
```json
{
  "success": false,
  "error": "错误消息",
  "code": "ERROR_CODE"
}
```

### 环境变量
```
DATABASE_URL=mysql://user:password@localhost:3306/astracup
OSU_CLIENT_ID=your_client_id
OSU_CLIENT_SECRET=your_client_secret
JWT_SECRET=your_jwt_secret
REDIS_URL=redis://localhost:6379 (可选，用于会话缓存)
```

## 兼容性考虑

### 数据迁移
1. 保持现有数据库表结构不变
2. 新Rust后端直接使用现有数据库
3. 逐步迁移API端点，保持前端兼容

### API兼容性
1. 保持相同的请求/响应格式
2. 保持相同的错误码
3. 逐步替换，可以并行运行

## 性能优势
1. **内存效率**: Rust无GC，内存占用更低
2. **并发性能**: 异步处理，高并发支持
3. **启动速度**: 编译为原生二进制，启动更快
4. **资源消耗**: CPU和内存使用更少

## 风险评估
1. **学习曲线**: Rust有一定学习成本
2. **迁移时间**: 需要2-3周完成迁移
3. **兼容性问题**: 需要确保API完全兼容
4. **测试覆盖**: 需要充分测试确保稳定性

## 下一步行动
1. 创建Rust项目
2. 配置基础依赖
3. 创建数据库模型
4. 实现第一个API端点作为示例