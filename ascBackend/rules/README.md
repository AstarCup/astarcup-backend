# 比赛规则目录

`/api/bot/rules` 在 `tournament_settings.rules` 为空时，会读取本目录下的 `{season}.md`。

- 文件名与赛季 ID 对应，例如 `s1.md`、`s2.md`；
- 内容为 Markdown，AstrBot 会原样交给模型阅读；
- 也可以通过后台 `PUT /api/settings` 写入 `rules` 设置项来覆盖文件内容（优先级更高）。

当前 `s1.md` / `s2.md` 是前端 `astarcup-rewireweb/src/data/guide/` 的副本，方便 Bot 开箱即用。更新比赛手册时请同步这两个文件，或者直接用 `rules` 设置项覆盖。
