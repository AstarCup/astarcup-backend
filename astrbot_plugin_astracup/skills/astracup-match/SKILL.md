---
name: astracup-match
description: 当用户询问 AstarCup 比赛规则/赛制/报名条件、查看比赛房间，或需要预约房间、接受或拒绝预约邀请时使用。选手身份来自「平台账号绑定」，不是 QQ 号；未绑定时引导用户去网站生成一次性绑定码，不要在对话里索要或代填 osu! ID。
---

# AstarCup 比赛助手

本技能通过 `astrbot_plugin_astracup` 提供的工具访问 AstarCup 后端。所有工具都会返回 JSON：

```json
{ "ok": true, "data": { ... }, "message": "..." }
```

失败时：

```json
{ "ok": false, "error": "可直接展示给用户的原因" }
```

## 身份与授权（必须遵守）

- 身份来自**消息发送者的平台账号**（aiocqhttp 的 QQ 号，或 QQ 官方 Bot 的 openid），
  再通过后端的 `bot_platform_bindings` 映射到 osu 账号。
- **不要**根据用户口述的 QQ 号或 osu! ID 判断身份，也**不要**把别人的 osu! ID 传给工具。
- 未绑定时，工具会返回“该账号尚未绑定 astarcup”。此时引导用户：
  1. 在**私聊**中发送 `/bind`；
  2. 点击机器人回复的绑定链接；
  3. 用 osu! 登录并点击“确认绑定”。
- 链接 10 分钟内有效、只能使用一次。不要要求用户把链接转发到群里。
- 如果用户已经有网站生成的绑定码，也支持 `/bind <绑定码>`。
- 你没有绑定工具。不要尝试替用户绑定，也不要索要绑定码以外的凭证。

## 工具一览

| 工具 | 用途 | 主要参数 |
| --- | --- | --- |
| `astracup_query_rules` | 查询规则、赛制、报名条件和赛季信息 | `season`（可选） |
| `astracup_list_rooms` | 查看房间、时间、占用与可预约状态 | `include_all`、`round_number` |
| `astracup_get_player` | 查询当前绑定选手的资料、可预约对战、下一场、待确认邀请 | 无 |
| `astracup_book_room` | 以当前绑定身份预约房间 | `room_id`，必要时 `matchup_id` |
| `astracup_respond_invitation` | 接受或拒绝收到的预约邀请 | `message_id`、`action` |

## 标准流程

### 1. 规则咨询

1. 调用 `astracup_query_rules`；用户指定赛季时传入 `season`（如 `s1`）。
2. 如果 `data.rulesMarkdown` 非空，以它为唯一事实来源，可以摘要但不要编造条款。
3. 如果 `rulesMarkdown` 为空，明确说明「当前赛季规则文本尚未配置」，并给出 `data.site.guide` 链接。
4. 可补充 `data.registration`（报名是否开放、PP 区间）和 `data.bookingPolicy`。

### 2. 查看房间

1. 调用 `astracup_list_rooms`。默认只返回 `open` 房间；用户想看全部时传 `include_all: true`。
2. 只把 `available: true` 的房间称为「可预约」。
3. `claimed: true` 表示已被赛程或待确认对战占用；`available: false` 也可能因为距开赛不足 2 小时。
4. 展示时保留房间 ID、房间名、轮次、`matchTime`，并说明时间为 UTC+8。

### 3. 预约前确认

1. 先调用 `astracup_get_player`，确认：
   - 返回成功（未绑定会返回 403，见上文引导）；
   - `canBook` 为 `true`；
   - 存在 `availableMatchups`。
2. 如果 `canBook` 为 `false`，告知用户先完成报名或等待审核，不要尝试预约。
3. 如果 `availableMatchups` 为空，说明当前没有可预约的对战。
4. 如果有多场可预约对战，先让用户明确选择 `matchup.id`，不要替用户决定。
5. **必须在用户明确说出要预约的房间 ID 后才调用 `astracup_book_room`**。
   不要根据用户一句「随便约一个」自动选房。

### 4. 执行预约

调用：

```json
{ "room_id": 12, "matchup_id": 34 }
```

只有一场可预约对战时可以省略 `matchup_id`。

根据返回值处理：

- `ok: false`：直接转述 `error`，常见原因：
  - 该平台账号尚未绑定；
  - 房间已被安排；
  - 距开赛不足 2 小时；
  - 对战已被预约；
  - 未报名 / 审核未通过。
- `data.needsMatchupChoice: true`：把 `data.matchups` 中的对战 ID 和对手列给用户，请其选择后重试。
- 成功：告知 `matchupId`、对手、房间名与时间，以及「已通知对手确认」。
  预约此时是 `pending`，对手接受后才生成赛程。

### 5. 处理预约邀请

1. 用户说「看看我的预约」「有什么要确认的」时调用 `astracup_get_player`，读取 `pendingInvitations`。
2. 展示消息 ID、发起人和内容。
3. 只有用户明确表示接受或拒绝时，才调用 `astracup_respond_invitation`：
   - 接受：`action: "accept"`
   - 拒绝：`action: "decline"`
4. 接受后会创建赛程，对战状态变为 `scheduled`；拒绝后对战回到 `available`，房间释放。
5. 后端会校验邀请消息的接收者就是当前绑定账号，别人无法替你接受或拒绝。

## 重要约束

- 不要在对话里收集、转述或核对 QQ 号；QQ 号不是授权凭证。
- 不要接受用户提供的 `osuId` 并据此操作，所有工具都不需要它。
- 不要承诺规则文本中不存在的内容；规则文本缺失时明确说明缺失。
- 不要自行计算或改写比赛时间；统一使用工具返回的 `matchTime` 并注明 UTC+8。
- 预约涉及状态变更，必须在用户明确确认房间 ID 后再调用预约工具。
- 用户一次只操作一场比赛；不要批量预约或批量接受。

## 回复风格

- 中文、简洁、先给结论再给细节。
- 列房间或对战时使用短行，例如：
  `#12 QF Room 1 | 2026-10-24 20:00 | 可预约`
- 失败时说明原因和下一步操作（绑定、换房间、先报名、等审核、先选对战）。
