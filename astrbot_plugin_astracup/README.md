# astrbot_plugin_astracup

AstrBot 插件：查询 AstarCup 比赛规则、查看可预约房间、以**已绑定的平台身份**预约比赛房间并处理预约确认。

## 1. 后端准备

在 `astarcup-backend` 的 `.env` 中新增：

```dotenv
BOT_TOKEN=换成一串足够长的随机字符串
BOT_RULES_DIR=./rules
```

重启后端。`BOT_TOKEN` 为空时，`/api/bot/*` 会返回 503。

后端启动时会幂等创建 `bot_platform_bindings` 和 `bot_bind_codes` 两张表；也可以手动执行：

```bash
psql "$DATABASE_URL" -f ascBackend/sql/bot_identity.sql
```

比赛规则来源（按优先级）：

1. `tournament_settings.rules`（管理员可通过 `PUT /api/settings` 写入）；
2. `BOT_RULES_DIR/{season}.md`，例如 `rules/s1.md`（可把前端的比赛手册复制过来）。

## 2. 安装插件

把本目录整体放到 AstrBot 的 `data/plugins/astrbot_plugin_astracup/`，在 WebUI 重载插件，然后填写插件配置：

- `base_url`：后端地址，例如 `https://api.example.com`
- `bot_token`：与后端 `BOT_TOKEN` 一致

生产环境请使用 HTTPS：`bot_token` 和绑定码都会随请求传输。

插件依赖 `aiohttp`（AstrBot 通常已自带，`requirements.txt` 中也已声明）。

## 3. 身份绑定（重要）

**不要用 QQ 号作为身份凭证。** QQ 官方 Bot 的 sender id 是 openid，不是 QQ 号；
而报名时填写的 QQ 是自填的，无法证明归属。插件采用平台身份绑定：

1. 用户在**私聊**中发送 `/bind`；
2. 机器人回复一次性绑定链接（`https://网站/bind#token=...`，10 分钟有效）；
3. 用户点击链接，用 osu! 登录后确认；
4. 后端把 `(platform, platformUserId)` 写入 `bot_platform_bindings`，映射到该用户的 osu 账号；
5. 之后所有查询/预约都只使用这个绑定关系，请求体里不接受 `osuId` 或 `qq`。

也可以在网站个人中心生成绑定码，然后私聊发送 `/bind <绑定码>`（旧流程）。

链接里的 token 放在 URL fragment 中，不会随 HTTP 请求发送到服务器，也不会进入访问日志；打开链接只展示确认页，必须登录并点击“确认绑定”才会消耗 token，因此 QQ 的链接预览不会误触。

平台身份：

- `platform` 形式为 `适配器名[:BotID]`，例如 `aiocqhttp:123456`、`qq_official:789`；
- `aiocqhttp`（NapCat / Lagrange 等）：`platformUserId` 是 QQ 号；
- `qq_official`：`platformUserId` 是 openid（每个 Bot 应用内稳定，因此 platform 里带上 BotID 避免冲突）。

授权边界：

- 机器人 token 只证明请求来自机器人，不证明用户身份；
- 用户身份来自平台适配器下发的 sender id，其他用户无法伪造；
- 预约时必须是对战参赛者，接受/拒绝时必须是消息接收者；
- 绑定操作在群聊中会被拒绝，避免链接或绑定码泄露。

如果用户未绑定，机器人会提示私聊发送 `/bind` 获取绑定链接。

## 4. 使用

指令：

| 指令 | 说明 |
| --- | --- |
| `/rules [赛季]` | 查询比赛规则与赛季信息 |
| `/rooms [all]` | 查看可预约房间；`all` 显示全部房间 |
| `/bind` | 私聊获取一键绑定链接（推荐） |
| `/bind <绑定码>` | 用网站生成的绑定码手动绑定（兼容） |
| `/me` | 查看自己的资料、可预约对战、下一场、待确认邀请 |
| `/book <房间ID> [对战ID]` | 预约房间；只有一场可约对战时对战 ID 可省略 |
| `/accept <消息ID>` | 接受对手发来的预约邀请 |
| `/decline <消息ID>` | 拒绝预约邀请 |

这些是顶层指令（不需要 `astracup` 前缀）。如果与其它插件重名，AstrBot 可能同时触发多个处理器，建议停用冲突插件或调整指令名。

LLM 工具（由 Agent 自动调用）：

- `astracup_query_rules`
- `astracup_list_rooms`
- `astracup_get_player`
- `astracup_book_room`
- `astracup_respond_invitation`

插件的 `skills/astracup-match/SKILL.md` 会随插件一起加载到 Skill Manager，用于指导模型何时调用这些工具。

## 5. 规则约束

预约复用网站同一套规则：

- 房间必须存在且状态为 `open`；
- 房间没有赛程，也没有 `pending` / `scheduled` 的对战；
- 距开赛不足 2 小时不能预约；
- 预约后对战进入 `pending`，对手需要接受（`accept`）才会生成赛程；
- 参赛者必须是当前绑定账号，未报名/未审核通过不能预约。

时间统一按 UTC+8 展示。
