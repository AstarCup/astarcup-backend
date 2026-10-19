from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass, field
from typing import Any

import aiohttp

from astrbot.api import AstrBotConfig, FunctionTool, logger
from astrbot.api.event import AstrMessageEvent, filter
from astrbot.api.star import Context, Star

DEFAULT_BASE_URL = "http://localhost:4000"
ROUND_LABELS = {
    1: "QUA",
    2: "RO16",
    3: "QF",
    4: "SF",
    5: "F",
    6: "GF",
}
STATUS_LABELS = {
    "not_registered": "未报名",
    "registered": "已报名（待审核）",
    "approved": "已通过",
}


class AstracupClient:
    """astarcup 后端 Bot API 的轻量异步客户端。"""

    def __init__(self, base_url: str, token: str, timeout: int = 10):
        raw_base = str(base_url or DEFAULT_BASE_URL).strip().rstrip("/")
        # 兼容把 /api 或 /api/bot 也填进来的配置；客户端会自己拼接 /api/bot/*
        for suffix in ("/api/bot", "/api"):
            if raw_base.endswith(suffix):
                raw_base = raw_base[: -len(suffix)].rstrip("/")
                break
        self.base_url = raw_base or DEFAULT_BASE_URL.rstrip("/")
        self.token = (token or "").strip()
        self.timeout = max(1, int(timeout or 10))
        self._session: aiohttp.ClientSession | None = None

    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=self.timeout),
                headers={
                    "X-Bot-Token": self.token,
                    "Accept": "application/json",
                },
            )
        return self._session

    async def close(self) -> None:
        if self._session is not None and not self._session.closed:
            await self._session.close()
        self._session = None

    async def request(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, Any] | None = None,
        json_body: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        if not self.token:
            return {"ok": False, "error": "插件未配置 bot_token，请联系管理员"}

        session = await self._get_session()
        url = f"{self.base_url}{path}"
        try:
            async with session.request(
                method, url, params=params, json=json_body
            ) as response:
                raw = await response.text()
                try:
                    payload = json.loads(raw)
                except json.JSONDecodeError:
                    logger.warning(
                        "astracup 返回非 JSON 响应: %s %s -> %s",
                        method,
                        path,
                        response.status,
                    )
                    return {
                        "ok": False,
                        "error": f"后端返回了非 JSON 响应（HTTP {response.status}）",
                    }
                if response.status >= 400 or not payload.get("success"):
                    return {
                        "ok": False,
                        "error": payload.get("error") or f"HTTP {response.status}",
                    }
                return {
                    "ok": True,
                    "data": payload.get("data"),
                    "message": payload.get("message"),
                }
        except asyncio.TimeoutError:
            return {"ok": False, "error": "请求 astarcup 后端超时"}
        except aiohttp.ClientError as exc:
            logger.warning("astracup 请求失败: %s", exc)
            return {"ok": False, "error": f"无法连接 astarcup 后端：{exc}"}

    async def rules(self, season: str | None = None) -> dict[str, Any]:
        params = {"season": season} if season else None
        return await self.request("GET", "/api/bot/rules", params=params)

    async def rooms(
        self, include_all: bool = False, round_number: int | None = None
    ) -> dict[str, Any]:
        params: dict[str, Any] = {}
        if include_all:
            params["all"] = "true"
        if round_number:
            params["round"] = round_number
        return await self.request("GET", "/api/bot/rooms", params=params or None)

    async def mappool(
        self, season: str | None = None, category: str | None = None
    ) -> dict[str, Any]:
        params: dict[str, Any] = {}
        if season:
            params["season"] = season
        if category:
            params["category"] = category
        return await self.request("GET", "/api/bot/mappool", params=params or None)

    async def settings(self) -> dict[str, Any]:
        return await self.request("GET", "/api/bot/settings")

    async def whoami(
        self, platform: str, platform_user_id: str
    ) -> dict[str, Any]:
        return await self.request(
            "GET",
            "/api/bot/whoami",
            params=_platform_params(platform, platform_user_id),
        )

    async def player(
        self, platform: str, platform_user_id: str
    ) -> dict[str, Any]:
        return await self.request(
            "GET",
            "/api/bot/players",
            params=_platform_params(platform, platform_user_id),
        )

    async def messages(
        self, platform: str, platform_user_id: str
    ) -> dict[str, Any]:
        return await self.request(
            "GET",
            "/api/bot/messages",
            params=_platform_params(platform, platform_user_id),
        )

    async def bind(
        self,
        *,
        platform: str,
        platform_user_id: str,
        code: str,
        display_name: str | None = None,
    ) -> dict[str, Any]:
        return await self.request(
            "POST",
            "/api/bot/bind",
            json_body={
                "platform": platform,
                "platformUserId": platform_user_id,
                "code": code,
                "displayName": display_name,
            },
        )

    async def create_bind_link(
        self,
        *,
        platform: str,
        platform_user_id: str,
        display_name: str | None = None,
    ) -> dict[str, Any]:
        return await self.request(
            "POST",
            "/api/bot/bind-link",
            json_body={
                "platform": platform,
                "platformUserId": platform_user_id,
                "displayName": display_name,
            },
        )

    async def book(
        self,
        *,
        platform: str,
        platform_user_id: str,
        room_id: int,
        matchup_id: int | None = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {
            "roomId": room_id,
            **_platform_params(platform, platform_user_id),
        }
        if matchup_id:
            body["matchupId"] = matchup_id
        return await self.request("POST", "/api/bot/book", json_body=body)

    async def respond(
        self,
        *,
        platform: str,
        platform_user_id: str,
        message_id: int,
        action: str,
    ) -> dict[str, Any]:
        body = {
            "messageId": message_id,
            "action": action,
            **_platform_params(platform, platform_user_id),
        }
        return await self.request("POST", "/api/bot/book/respond", json_body=body)


def _platform_params(platform: str, platform_user_id: str) -> dict[str, str]:
    return {"platform": platform, "platformUserId": platform_user_id}


def _sender_id(event: AstrMessageEvent) -> str:
    try:
        return str(event.get_sender_id() or "").strip()
    except Exception:  # noqa: BLE001 - 不同适配器的字段可能缺失
        return ""


def _platform_identity(event: AstrMessageEvent) -> tuple[str, str]:
    """平台身份：platform 用「适配器名[:BotID]」，platformUserId 用平台原始 ID。

    QQ 官方 Bot 的 sender id 是 openid，且只在同一个 Bot 应用内唯一；
    aiocqhttp 则是 QQ 号。把 Bot 自身 ID 纳入 platform，可避免多 Bot 部署时冲突。
    后端只认绑定关系，不依赖真实 QQ 号。
    """
    try:
        platform_name = str(event.get_platform_name() or "").strip()
    except Exception:  # noqa: BLE001
        platform_name = ""
    platform_name = platform_name or "unknown"

    self_id = ""
    try:
        message_obj = getattr(event, "message_obj", None)
        self_id = str(getattr(message_obj, "self_id", "") or "").strip()
    except Exception:  # noqa: BLE001
        self_id = ""

    platform = f"{platform_name}:{self_id}" if self_id else platform_name
    return platform, _sender_id(event)


def _dump(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, default=str)


def _round_label(round_number: Any) -> str:
    try:
        return ROUND_LABELS.get(int(round_number), "Stage")
    except (TypeError, ValueError):
        return "Stage"


def _room_line(room: dict[str, Any]) -> str:
    if room.get("available"):
        state = "可预约"
    elif room.get("claimed"):
        state = "已被占用"
    else:
        state = "不可预约"
    return (
        f'#{room.get("id")} {room.get("roomName", "")} '
        f'| {room.get("roundLabel") or _round_label(room.get("roundNumber"))} '
        f'| {room.get("matchTime", "")} | {state}'
    )


def _fmt_length(seconds: Any) -> str:
    try:
        total = int(seconds)
    except (TypeError, ValueError):
        return "-"
    return f"{total // 60}:{total % 60:02d}"


def _bind_hint() -> str:
    return (
        "你还没有绑定 astarcup 账号。\n"
        "1. 在私聊中发送：/bind\n"
        "2. 点击机器人回复的绑定链接\n"
        "3. 用 osu! 登录并点击「确认绑定」\n"
        "链接 10 分钟内有效，只能使用一次。"
    )


class AstracupPlugin(Star):
    def __init__(self, context: Context, config: AstrBotConfig):
        super().__init__(context)
        self.config = config
        self.client = AstracupClient(
            config.get("base_url", DEFAULT_BASE_URL),
            config.get("bot_token", ""),
            config.get("timeout", 10),
        )
        self.context.add_llm_tools(
            QueryRulesTool(client=self.client),
            GetSettingsTool(client=self.client),
            QueryMappoolTool(client=self.client),
            ListRoomsTool(client=self.client),
            GetPlayerTool(client=self.client),
            BookRoomTool(client=self.client),
            RespondInvitationTool(client=self.client),
        )

    async def terminate(self) -> None:
        await self.client.close()

    # ============ 指令 ============

    @filter.command("rules")
    async def rules_command(self, event: AstrMessageEvent, season: str = ""):
        """查询比赛规则：/rules [赛季]"""
        season = (season or "").strip()
        result = await self.client.rules(season or None)
        if not result.get("ok"):
            yield event.plain_result(f'查询失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        title = data.get("tournamentName") or "AstarCup"
        season_id = str(data.get("season") or "").upper()
        season_name = data.get("seasonName")
        lines = [f"{title} · {season_id}" + (f"（{season_name}）" if season_name else "")]
        registration = data.get("registration") or {}
        pp_range = registration.get("ppRange") or {}
        lines.append(
            "报名："
            + ("开放" if registration.get("enabled") else "未开放")
            + f'，PP {pp_range.get("min", 0):g} - {pp_range.get("max", 0):g}'
        )
        rules = data.get("rulesMarkdown")
        if rules:
            text = str(rules).strip()
            if len(text) > 1800:
                text = text[:1800] + "\n……（规则较长，完整内容见网站）"
            lines.append("")
            lines.append(text)
        else:
            lines.append("当前赛季尚未配置规则文本，可查看网站比赛手册。")
        site = data.get("site") or {}
        if site.get("guide"):
            lines.append(f'完整手册：{site["guide"]}')
        yield event.plain_result("\n".join(lines))

    @filter.command("rooms")
    async def rooms_command(self, event: AstrMessageEvent, scope: str = ""):
        """查看可预约房间：/rooms [all]"""
        include_all = (scope or "").strip().lower() in {"all", "全部"}
        result = await self.client.rooms(include_all=include_all)
        if not result.get("ok"):
            yield event.plain_result(f'查询失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        rooms = data.get("rooms") or []
        if not rooms:
            yield event.plain_result("当前没有可显示的房间。")
            return
        header = (
            f'共 {data.get("total", len(rooms))} 个房间，'
            f'可预约 {data.get("availableCount", 0)} 个（时间为 UTC+8）'
        )
        lines = [header]
        lines.extend(_room_line(room) for room in rooms[:20])
        if len(rooms) > 20:
            lines.append(f"……仅显示前 20 个，共 {len(rooms)} 个")
        lines.append("预约：/book <房间ID> [对战ID]")
        yield event.plain_result("\n".join(lines))

    @filter.command("mappool", alias={"pool", "图池"})
    async def mappool_command(
        self, event: AstrMessageEvent, season: str = "", category: str = ""
    ):
        """查看公开图池：/mappool [赛季] [阶段]（阶段：QUA/RO16/QF/SF/F/GF）"""
        season = (season or "").strip()
        category = (category or "").strip()
        # 允许 /mappool QF 这种只写阶段的用法
        category_words = {
            "all",
            "qua",
            "qualification",
            "ro32",
            "ro16",
            "qf",
            "quarterfinals",
            "sf",
            "semifinals",
            "f",
            "finals",
            "gf",
            "grandfinals",
            "全部",
            "资格赛",
            "三十二强",
            "十六强",
            "八强",
            "四分之一决赛",
            "半决赛",
            "决赛",
            "总决赛",
        }
        if season.lower() in category_words and not category:
            season, category = "", season
        result = await self.client.mappool(season or None, category or None)
        if not result.get("ok"):
            yield event.plain_result(f'查询失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        if not data.get("visible"):
            yield event.plain_result(data.get("message") or "图池尚未公开。")
            return

        maps = data.get("maps") or []
        header = (
            f'{data.get("tournamentName") or "AstarCup"} '
            f'{str(data.get("season") or "").upper()} · '
            f'{data.get("categoryLabel") or "全部"} 图池（{data.get("count", len(maps))} 张）'
        )
        if not maps:
            yield event.plain_result(header + "\n该阶段暂无公开图池。")
            return

        lines = [header]
        for group in data.get("groups") or []:
            lines.append(f'[{group.get("label")}]')
            for mod_group in group.get("mods") or []:
                lines.append(f'  {mod_group.get("mod")}:')
                for m in mod_group.get("maps") or []:
                    star = m.get("starRating")
                    star_text = f"{star:g}" if isinstance(star, (int, float)) else "-"
                    lines.append(
                        f'    {m.get("position", 0)}. {m.get("artist")} - {m.get("title")} '
                        f'[{m.get("version")}] ★{star_text} '
                        f'· {_fmt_length(m.get("lengthSeconds"))}'
                    )
        text = "\n".join(lines)
        if len(text) > 1800:
            text = text[:1800] + "\n……（图池较长，完整内容见网站）"
        site = data.get("site") or {}
        if site.get("mappool"):
            text += f'\n完整图池：{site["mappool"]}'
        yield event.plain_result(text)

    @filter.command("settings", alias={"设置"})
    async def settings_command(self, event: AstrMessageEvent):
        """查看全局比赛设置：/settings"""
        result = await self.client.settings()
        if not result.get("ok"):
            yield event.plain_result(f'查询失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        registration = data.get("registration") or {}
        pp_range = registration.get("ppRange") or {}
        season = str(data.get("currentSeason") or "").upper()
        season_name = data.get("currentSeasonName")

        lines = [
            f'{data.get("tournamentName") or "AstarCup"}',
            f'当前赛季：{season}' + (f'（{season_name}）' if season_name else ""),
            f'当前阶段：{data.get("currentStageLabel") or data.get("currentStage") or "-"}',
            "报名："
            + ("开放" if registration.get("enabled") else "未开放")
            + f'，PP {pp_range.get("min", 0):g} - {pp_range.get("max", 0):g}',
            "公开图池：" + ("已公开" if data.get("mappoolVisible") else "未公开"),
        ]
        seasons = data.get("seasons") or []
        if seasons:
            lines.append(
                "赛季："
                + "、".join(
                    f'{s.get("name") or s.get("id")}（{s.get("status") or "-"}）'
                    for s in seasons
                )
            )
        site = data.get("site") or {}
        if site.get("guide"):
            lines.append(f'比赛手册：{site["guide"]}')
        yield event.plain_result("\n".join(lines))

    @filter.command("bind")
    async def bind_command(self, event: AstrMessageEvent, code: str = ""):
        """绑定网站账号：/bind（发送一键链接）或 /bind <绑定码>"""
        try:
            group_id = event.get_group_id()
        except Exception:  # noqa: BLE001
            group_id = ""
        if group_id:
            yield event.plain_result("请在私聊中绑定，避免绑定链接或绑定码泄露到群里。")
            return

        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return
        try:
            display_name = str(event.get_sender_name() or "")[:64]
        except Exception:  # noqa: BLE001
            display_name = ""

        # 不带参数：直接发一次性绑定链接，点击后登录 osu! 确认
        code = (code or "").strip()
        if not code:
            result = await self.client.create_bind_link(
                platform=platform,
                platform_user_id=platform_user_id,
                display_name=display_name or None,
            )
            if not result.get("ok"):
                yield event.plain_result(f'生成绑定链接失败：{result.get("error")}')
                return
            data = result.get("data") or {}
            yield event.plain_result(
                "点击下面的链接完成绑定（10 分钟内有效，仅限本人）：\n"
                f'{data.get("url")}\n'
                "打开后使用 osu! 登录并确认即可。请勿转发该链接。"
            )
            return

        # 兼容手动绑定码流程
        result = await self.client.bind(
            platform=platform,
            platform_user_id=platform_user_id,
            code=code,
            display_name=display_name or None,
        )
        if not result.get("ok"):
            yield event.plain_result(f'绑定失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        yield event.plain_result(
            f'绑定成功：{data.get("username")}（osu! ID {data.get("osuId")}）。\n'
            "现在可以使用 /me 查看比赛信息。"
        )

    @filter.command("me")
    async def me_command(self, event: AstrMessageEvent):
        """查看我的比赛信息：/me"""
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return

        who = await self.client.whoami(platform, platform_user_id)
        if not who.get("ok"):
            yield event.plain_result(f'查询失败：{who.get("error")}')
            return
        if not (who.get("data") or {}).get("bound"):
            yield event.plain_result(_bind_hint())
            return

        result = await self.client.player(platform, platform_user_id)
        if not result.get("ok"):
            yield event.plain_result(f'查询失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        lines = [
            f'{data.get("username")}（osu! ID {data.get("osuId")}）',
            f'报名状态：{STATUS_LABELS.get(str(data.get("registrationStatus")), data.get("registrationStatus"))}',
        ]
        matchups = data.get("availableMatchups") or []
        if matchups:
            lines.append("可预约对战：")
            for matchup in matchups:
                opponent = matchup.get("opponent") or {}
                lines.append(
                    f'  对战 #{matchup.get("id")} vs {opponent.get("username")}'
                )
        else:
            lines.append("可预约对战：无")
        next_schedule = data.get("nextSchedule")
        if next_schedule:
            room = next_schedule.get("room") or {}
            lines.append(
                f'下一场：赛程 #{next_schedule.get("id")} '
                f'| {room.get("roomName", "未安排房间")} '
                f'| {room.get("matchTime", "")}'
            )
        invitations = data.get("pendingInvitations") or []
        if invitations:
            lines.append("待确认预约：")
            for message in invitations:
                lines.append(
                    f'  消息 #{message.get("id")}：{message.get("content")}'
                )
            lines.append("接受：/accept <消息ID>；拒绝：/decline <消息ID>")
        yield event.plain_result("\n".join(lines))

    @filter.command("book")
    async def book_command(
        self, event: AstrMessageEvent, room_id: int, matchup_id: int = 0
    ):
        """预约房间：/book <房间ID> [对战ID]"""
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return
        result = await self.client.book(
            platform=platform,
            platform_user_id=platform_user_id,
            room_id=room_id,
            matchup_id=matchup_id or None,
        )
        yield event.plain_result(self._format_booking(result))

    @filter.command("accept")
    async def accept_command(self, event: AstrMessageEvent, message_id: int):
        """接受预约邀请：/accept <消息ID>"""
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return
        result = await self.client.respond(
            platform=platform,
            platform_user_id=platform_user_id,
            message_id=message_id,
            action="accept",
        )
        yield event.plain_result(self._format_response(result))

    @filter.command("decline")
    async def decline_command(self, event: AstrMessageEvent, message_id: int):
        """拒绝预约邀请：/decline <消息ID>"""
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return
        result = await self.client.respond(
            platform=platform,
            platform_user_id=platform_user_id,
            message_id=message_id,
            action="decline",
        )
        yield event.plain_result(self._format_response(result))

    # ============ 文本格式化 ============

    @staticmethod
    def _format_booking(result: dict[str, Any]) -> str:
        if not result.get("ok"):
            return f'预约失败：{result.get("error")}'
        data = result.get("data") or {}
        if data.get("needsMatchupChoice"):
            lines = ["你有多个可预约对战，请指定对战 ID 后重试："]
            for matchup in data.get("matchups") or []:
                opponent = matchup.get("opponent") or {}
                lines.append(
                    f'  对战 #{matchup.get("id")} vs {opponent.get("username")}'
                )
            lines.append("用法：/book <房间ID> <对战ID>")
            return "\n".join(lines)
        opponent = data.get("opponent") or {}
        room = data.get("room") or {}
        return (
            "预约成功，已通知对手确认。\n"
            f'对战 #{data.get("matchupId")} vs {opponent.get("username")}\n'
            f'房间：{room.get("roomName")}（{room.get("matchTime")}，UTC+8）\n'
            f'对手确认消息 ID：{data.get("messageId")}'
        )

    @staticmethod
    def _format_response(result: dict[str, Any]) -> str:
        if not result.get("ok"):
            return f'操作失败：{result.get("error")}'
        data = result.get("data") or {}
        action = "接受" if data.get("action") == "accept" else "拒绝"
        return (
            f'已{action}预约（消息 #{data.get("messageId")}）。\n'
            f'对战 #{data.get("matchupId")} 状态：{data.get("matchupStatus", "-")}'
        )


# ============ LLM 工具 ============


@dataclass
class QueryRulesTool(FunctionTool):
    name: str = "astracup_query_rules"
    description: str = (
        "查询 AstarCup 的比赛规则、赛制、报名条件和当前赛季信息。"
        "当用户询问规则、赛制、报名要求、比赛时间安排时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "season": {
                    "type": "string",
                    "description": "赛季 ID，例如 s1；留空表示当前赛季",
                }
            },
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(self, event: AstrMessageEvent, season: str = "") -> str:
        return _dump(await self.client.rules((season or "").strip() or None))


@dataclass
class GetSettingsTool(FunctionTool):
    name: str = "astracup_get_settings"
    description: str = (
        "查询 AstarCup 全局比赛设置：当前赛季/阶段、报名开关、PP 区间、图池是否公开、赛季列表。"
        "用户问「现在什么阶段」「报名条件」「图池公开了吗」时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {"type": "object", "properties": {}}
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(self, event: AstrMessageEvent) -> str:
        return _dump(await self.client.settings())


@dataclass
class QueryMappoolTool(FunctionTool):
    name: str = "astracup_query_mappool"
    description: str = (
        "查询 AstarCup 公开图池（只包含已公开且已过审的图）。"
        "用户问「图池是什么」「QF 有哪些图」「某赛季图池」时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "season": {
                    "type": "string",
                    "description": "赛季 ID，例如 s2；留空表示当前赛季",
                },
                "category": {
                    "type": "string",
                    "description": "阶段：all/QUA/RO16/QF/SF/F/GF；留空表示当前阶段",
                },
            },
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        season: str = "",
        category: str = "",
    ) -> str:
        return _dump(
            await self.client.mappool(
                (season or "").strip() or None,
                (category or "").strip() or None,
            )
        )


@dataclass
class ListRoomsTool(FunctionTool):
    name: str = "astracup_list_rooms"
    description: str = (
        "列出 AstarCup 的比赛房间、时间、占用状态和是否可预约。"
        "用户询问有哪些房间、还能约哪个时间时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "include_all": {
                    "type": "boolean",
                    "description": "是否包含不可预约的房间，默认 false",
                },
                "round_number": {
                    "type": "integer",
                    "description": "轮次编号：1=QUA, 2=RO16, 3=QF, 4=SF, 5=F, 6=GF",
                },
            },
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        include_all: bool = False,
        round_number: int = 0,
    ) -> str:
        return _dump(
            await self.client.rooms(
                include_all=bool(include_all),
                round_number=round_number or None,
            )
        )


@dataclass
class GetPlayerTool(FunctionTool):
    name: str = "astracup_get_player"
    description: str = (
        "查询当前消息发送者绑定的 AstarCup 选手资料、可预约对战、下一场比赛和待确认邀请。"
        "用户询问「我是谁」「我下一场什么时候」「我要约哪场」时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {"type": "object", "properties": {}}
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(self, event: AstrMessageEvent) -> str:
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        return _dump(await self.client.player(platform, platform_user_id))


@dataclass
class BookRoomTool(FunctionTool):
    name: str = "astracup_book_room"
    description: str = (
        "以当前消息发送者绑定的 AstarCup 身份预约房间。必须先确认房间 ID 和用户意图；"
        "若返回 needsMatchupChoice，需要让用户选择对战 ID 后再次调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "room_id": {
                    "type": "integer",
                    "description": "要预约的房间 ID",
                },
                "matchup_id": {
                    "type": "integer",
                    "description": "对战 ID；用户只有一场可预约对战时可省略",
                },
            },
            "required": ["room_id"],
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        room_id: int,
        matchup_id: int = 0,
    ) -> str:
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        return _dump(
            await self.client.book(
                platform=platform,
                platform_user_id=platform_user_id,
                room_id=room_id,
                matchup_id=matchup_id or None,
            )
        )


@dataclass
class RespondInvitationTool(FunctionTool):
    name: str = "astracup_respond_invitation"
    description: str = (
        "接受或拒绝当前消息发送者收到的 AstarCup 预约邀请。"
        "用户明确表示同意或拒绝某个邀请时调用。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "message_id": {
                    "type": "integer",
                    "description": "邀请消息 ID",
                },
                "action": {
                    "type": "string",
                    "enum": ["accept", "decline"],
                    "description": "accept 接受，decline 拒绝",
                },
            },
            "required": ["message_id", "action"],
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        message_id: int,
        action: str,
    ) -> str:
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        if action not in {"accept", "decline"}:
            return _dump({"ok": False, "error": "action 只能是 accept 或 decline"})
        return _dump(
            await self.client.respond(
                platform=platform,
                platform_user_id=platform_user_id,
                message_id=message_id,
                action=action,
            )
        )
