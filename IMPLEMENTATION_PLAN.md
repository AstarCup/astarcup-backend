# AstraCup Backend - CRUD 实施计划

## 项目概述
为AstraCup比赛管理后端生成完整的CRUD API，包括认证、授权和系统初始化功能。

## 技术栈
- **框架**: Axum 0.8.8
- **数据库**: SeaORM 1.1.19 + MySQL
- **认证**: JWT + osu! OAuth 2.0
- **文档**: utoipa (OpenAPI/Swagger)
- **验证**: validator 0.20
- **配置**: dotenv + config

## 实施阶段

### 第一阶段：基础架构搭建 ✅
- [x] 创建项目目录结构
- [x] 实现数据库连接池
- [x] 配置管理系统
- [x] 错误处理系统
- [x] 基础DTO和Service模板

### 第二阶段：认证系统 🔄
- [ ] JWT认证中间件
- [ ] osu! OAuth集成（含refresh token存储）
- [ ] 用户注册/登录流程
- [ ] 权限中间件（基于UserGroup）

### 第三阶段：核心CRUD
- [ ] User API完整实现
- [ ] 管理员设置功能
- [ ] 系统初始化逻辑
- [ ] TournamentSettings API

### 第四阶段：其他Entity CRUD
- [ ] MatchRooms API
- [ ] MatchSchedules API
- [ ] PlayerMatchups API
- [ ] MapSelections API
- [ ] MapComments API
- [ ] Messages API

### 第五阶段：文档和优化
- [ ] OpenAPI文档生成
- [ ] 测试编写
- [ ] 性能优化
- [ ] 部署配置

## 详细任务分解

### 第一阶段任务
1. **目录结构创建**
   - [x] 创建 `src/api/` 目录和模块
   - [x] 创建 `src/service/` 目录和模块
   - [x] 创建 `src/dto/` 目录和模块
   - [x] 创建 `src/error/` 目录和模块
   - [x] 创建 `src/config/` 目录和模块
   - [x] 创建 `src/middleware/` 目录和模块
   - [x] 创建 `src/utils/` 目录和模块

2. **数据库连接**
   - [x] 实现Database配置
   - [x] 创建连接池管理
   - [x] 添加数据库迁移支持

3. **错误处理**
   - [x] 定义AppError枚举
   - [x] 实现From trait用于错误转换
   - [x] 创建统一的错误响应格式

4. **配置管理**
   - [x] 从.env读取配置
   - [x] 创建Config结构体
   - [x] 实现配置验证

### 第二阶段任务
1. **JWT认证**
   - [ ] 创建JWT工具函数
   - [ ] 实现Token生成和验证
   - [ ] 创建认证中间件

2. **osu! OAuth集成**
   - [ ] 添加OAuth相关字段到User entity
   - [ ] 实现OAuth授权流程
   - [ ] 处理OAuth回调
   - [ ] 存储access/refresh token

3. **权限管理**
   - [ ] 创建权限检查中间件
   - [ ] 实现基于UserGroup的权限控制
   - [ ] 管理员权限特殊处理

### 第三阶段任务
1. **User API**
   - [ ] 创建User DTOs
   - [ ] 实现UserService
   - [ ] 创建User API路由
   - [ ] 添加数据验证

2. **系统初始化**
   - [ ] 检查tournament_settings表
   - [ ] 创建默认比赛设置
   - [ ] 管理员用户初始化逻辑

3. **TournamentSettings API**
   - [ ] 创建TournamentSettings DTOs
   - [ ] 实现TournamentSettingsService
   - [ ] 创建API路由

### 第四阶段任务
1. **MatchRooms API**
   - [ ] 创建MatchRooms DTOs
   - [ ] 实现MatchRoomsService
   - [ ] 创建API路由

2. **其他Entity API**（类似模式）
   - [ ] MatchSchedules
   - [ ] PlayerMatchups
   - [ ] MapSelections
   - [ ] MapComments
   - [ ] Messages

### 第五阶段任务
1. **文档生成**
   - [ ] 集成utoipa
   - [ ] 为所有API添加OpenAPI注解
   - [ ] 设置Swagger UI路由

2. **测试**
   - [ ] 单元测试
   - [ ] 集成测试
   - [ ] API测试

3. **优化**
   - [ ] 性能优化
   - [ ] 错误处理优化
   - [ ] 日志优化

## 数据库变更

### User表新增字段
```sql
ALTER TABLE user 
ADD COLUMN osu_access_token VARCHAR(512) NULL,
ADD COLUMN osu_refresh_token VARCHAR(512) NULL,
ADD COLUMN osu_token_expires_at TIMESTAMP NULL;
```

## API端点设计

### 认证相关
- `GET /api/auth/osu` - 重定向到osu! OAuth
- `GET /api/auth/callback/osu` - OAuth回调
- `POST /api/auth/refresh` - 刷新JWT token
- `POST /api/auth/logout` - 登出

### 用户管理
- `GET /api/users` - 获取用户列表（管理员）
- `GET /api/users/{id}` - 获取用户详情
- `PUT /api/users/{id}` - 更新用户信息
- `PUT /api/users/{id}/user-group` - 修改用户组（管理员）
- `DELETE /api/users/{id}` - 删除用户（管理员）

### 比赛管理
- `GET /api/match-rooms` - 获取比赛房间列表
- `POST /api/match-rooms` - 创建比赛房间
- `GET /api/match-rooms/{id}` - 获取比赛房间详情
- `PUT /api/match-rooms/{id}` - 更新比赛房间
- `DELETE /api/match-rooms/{id}` - 删除比赛房间

## 权限规则

### 用户角色
1. **玩家 (Player)**
   - 查看公开信息
   - 管理自己的数据
   - 参与比赛

2. **管理员 (Admin)**
   - 所有玩家权限
   - 管理所有用户
   - 修改系统设置
   - 管理比赛

### 权限矩阵
| 操作 | 玩家 | 管理员 |
|------|------|--------|
| 查看用户列表 | ❌ | ✅ |
| 修改用户组 | ❌ | ✅ |
| 创建比赛房间 | ✅ | ✅ |
| 删除比赛房间 | 仅自己 | ✅ |
| 修改系统设置 | ❌ | ✅ |

## 开发注意事项

1. **安全性**
   - 所有密码/Token加密存储
   - JWT secret从环境变量读取
   - 输入验证和清理
   - SQL注入防护

2. **性能**
   - 数据库连接池
   - 查询优化
   - 缓存策略

3. **可维护性**
   - 清晰的错误信息
   - 完整的日志记录
   - 统一的代码风格
   - 详细的API文档

