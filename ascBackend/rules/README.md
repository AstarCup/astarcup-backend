# 比赛规则目录

`/api/bot/rules` 的规则文本按以下优先级取值（见 `routes/bot_routes.rs::bot_rules`）：

1. `tournament_settings.rules` 设置项（全局覆盖，优先级最高）；
2. **`season_rules` 数据表**（正式存放位置，可在网站后台「内容管理 → 赛季规则」编辑）；
3. 本目录下的 `{season}.md` 文件（兜底，仅当数据库中没有该赛季的记录时读取）。

- 文件名与赛季 ID 对应，例如 `s1.md`、`s2.md`；
- 内容为 Markdown，AstrBot 会原样交给模型阅读；
- 规则已迁移到数据库：初始化/迁移请执行 `sql/content_tables.sql`（含 s1/s2 播种，幂等可重复执行）。
  播种只在首次建库时生效，已存在的行不会被覆盖；改文案后请在后台编辑，或先 `DELETE FROM season_rules WHERE season = '...'` 再重跑。

本目录的 `s1.md` / `s2.md` 是历史副本（也是前端 `astarcup-rewireweb/src/data/guide/` 的副本），现在仅作兜底保留。数据库有记录后，更新比赛规则请直接在后台编辑，无需再改文件。
