# astarcup-backend

AstraCup 的 Rust / axum 后端。

## 目录

- `ascBackend/`：后端服务（Cargo 项目）
- `astrbot_plugin_astracup/`：AstrBot 插件（规则查询、比赛预约、平台身份绑定）

## AstrBot 插件

QQ 机器人插件的安装、配置与身份绑定流程见
[`astrbot_plugin_astracup/README.md`](astrbot_plugin_astracup/README.md)。

## 数据库迁移

schema 变更的**单一事实源**是 `ascBackend/sql/*.sql`，由 `src/db.rs` 的 `MIGRATIONS`
登记并在后端启动时自动执行（记录写入 `schema_migrations` 表，每个文件只执行一次）：

- **新增迁移**：在 `ascBackend/sql/` 新建幂等脚本（建议 `IF NOT EXISTS` 风格，便于手动重放），
  然后在 `src/db.rs` 的 `MIGRATIONS` 里登记一行（`include_str!`，随二进制编译，部署不依赖外部文件）；
- **新库引导**：全自动——`dev_schema.sql` 全量基线仅在空库执行；既有库自动登记为已应用；
- **失败策略**：单个迁移失败只记警告、不阻塞启动，未记录的迁移下次启动自动重试；
- 也可手动执行：`sh sql/psql.sh -f sql/xxx.sql`。

## 部署注意

- **单实例假设**：后台任务（自动关房、用户 cover 刷新）与出站代理探测都是单实例语义，
  多副本部署会重复执行；如需横向扩容，先给任务加 advisory lock；
- **连接池**：默认 20 个连接，可用环境变量 `DB_POOL_SIZE` 调整；
- 配置项清单见 `ascBackend/.env.example`。
