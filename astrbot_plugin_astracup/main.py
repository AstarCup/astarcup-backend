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

    async def schedules(
        self,
        platform: str,
        platform_user_id: str,
        *,
        status: str | None = None,
        season: str | None = None,
        limit: int = 30,
    ) -> dict[str, Any]:
        params: dict[str, Any] = {
            **_platform_params(platform, platform_user_id),
            "limit": max(1, min(int(limit or 30), 100)),
        }
        if status:
            params["status"] = status
        if season:
            params["season"] = season
        return await self.request("GET", "/api/bot/schedules", params=params)

    async def record_score(
        self,
        *,
        platform: str,
        platform_user_id: str,
        schedule_id: int,
        red_score: int,
        blue_score: int,
        status: str | None = None,
        season: str | None = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {
            "platform": platform,
            "platformUserId": platform_user_id,
            "scheduleId": schedule_id,
            "redScore": red_score,
            "blueScore": blue_score,
        }
        if status:
            body["status"] = status
        if season:
            body["season"] = season
        return await self.request("POST", "/api/bot/scores", json_body=body)

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
            ListSchedulesTool(client=self.client),
            GetPlayerTool(client=self.client),
            BookRoomTool(client=self.client),
            RecordScoreTool(client=self.client),
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
        matchups = data.get("matchups") or []
        if matchups:
            lines.append("对战安排（红方=player1，蓝方=player2）：")
            for matchup in matchups:
                player1 = matchup.get("player1") or {}
                player2 = matchup.get("player2") or {}
                side = matchup.get("mySide")
                side_text = (
                    "你是红方"
                    if side == "red"
                    else "你是蓝方"
                    if side == "blue"
                    else "非参赛者"
                )
                room = matchup.get("room") or {}
                room_text = (
                    f' | 房间 {room.get("roomName")}' if room.get("roomName") else ""
                )
                lines.append(
                    f'  #{matchup.get("id")} {player1.get("username")}（红） vs '
                    f'{player2.get("username")}（蓝） | {side_text} | '
                    f'{matchup.get("status")}{room_text}'
                )
            available = data.get("availableMatchups") or []
            if available:
                ids = "、".join(str(m.get("id")) for m in available)
                lines.append(f"可预约对战 ID：{ids}")
                lines.append("预约：先 /book <房间ID> 查看安排，再 /book <房间ID> <对战ID>")
        else:
            lines.append("对战安排：暂无（请联系管理员）")
        next_schedule = data.get("nextSchedule")
        if next_schedule:
            room = next_schedule.get("room") or {}
            season = next_schedule.get("season")
            season_text = f"[{str(season).upper()}] " if season else ""
            lines.append(
                f'下一场：{season_text}赛程 #{next_schedule.get("id")} '
                f'{next_schedule.get("player1Username", "?")}（红） vs '
                f'{next_schedule.get("player2Username", "?")}（蓝） '
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
        """预约房间：/book <房间ID> 查看对战安排；/book <房间ID> <对战ID> 确认预约"""
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

    @filter.command("score", alias={"比分"})
    async def score_command(
        self,
        event: AstrMessageEvent,
        schedule_id: int = 0,
        red_score: int = -1,
        blue_score: int = -1,
        status: str = "",
        season: str = "",
    ):
        """管理员录入比分：/score [赛程ID 红方 蓝方 [状态] [赛季]]"""
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            yield event.plain_result("无法获取你的平台账号 ID。")
            return

        schedule_id = int(schedule_id or 0)
        red = -1 if red_score is None else int(red_score)
        blue = -1 if blue_score is None else int(blue_score)

        # 不带参数：列出最近赛程，方便管理员拿赛程 ID
        if schedule_id <= 0:
            result = await self.client.schedules(
                platform, platform_user_id, limit=20
            )
            if not result.get("ok"):
                yield event.plain_result(f'查询失败：{result.get("error")}')
                return
            yield event.plain_result(
                self._format_schedule_list(result.get("data") or {})
            )
            return

        if red < 0 or blue < 0:
            yield event.plain_result(
                "用法：/score <赛程ID> <红方比分> <蓝方比分> [状态] [赛季]"
            )
            return

        result = await self.client.record_score(
            platform=platform,
            platform_user_id=platform_user_id,
            schedule_id=schedule_id,
            red_score=red,
            blue_score=blue,
            status=(status or "").strip() or None,
            season=(season or "").strip() or None,
        )
        if not result.get("ok"):
            yield event.plain_result(f'记录失败：{result.get("error")}')
            return
        data = result.get("data") or {}
        player1 = data.get("player1") or {}
        player2 = data.get("player2") or {}
        season_text = (
            f'[{str(data.get("season")).upper()}] ' if data.get("season") else ""
        )
        yield event.plain_result(
            f'已记录 {season_text}#{data.get("scheduleId")}：'
            f'{player1.get("username")}（红） {data.get("redScore")} : '
            f'{data.get("blueScore")} {player2.get("username")}（蓝），'
            f'状态 {data.get("status")}。\n'
            "如果填反了，用同一命令重新提交即可覆盖。"
        )

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
    def _format_schedule_list(data: dict[str, Any]) -> str:
        schedules = data.get("schedules") or []
        if not schedules:
            return "暂无赛程。"
        lines = ["最近赛程（管理员录入比分）："]
        for s in schedules[:20]:
            player1 = s.get("player1") or {}
            player2 = s.get("player2") or {}
            season = s.get("season")
            season_text = str(season).upper() if season else "未标注"
            lines.append(
                f'#{s.get("id")} {season_text} {s.get("roundLabel")} {s.get("roomName")} | '
                f'{player1.get("username")}（红） {s.get("redScore")} : '
                f'{s.get("blueScore")} {player2.get("username")}（蓝） | '
                f'{s.get("status")} | {s.get("matchTime")}'
            )
        lines.append("用法：/score <赛程ID> <红方比分> <蓝方比分> [状态] [赛季]")
        lines.append("红方=player1，蓝方=player2；状态默认 completed。")
        return "\n".join(lines)

    @staticmethod
    def _format_booking(result: dict[str, Any]) -> str:
        if not result.get("ok"):
            return f'预约失败：{result.get("error")}'
        data = result.get("data") or {}
        if data.get("needsMatchupChoice"):
            lines = [
                "请先确认对战安排，再指定 matchupId 预约（红方=player1，蓝方=player2）："
            ]
            for matchup in data.get("matchups") or []:
                player1 = matchup.get("player1") or {}
                player2 = matchup.get("player2") or {}
                side = matchup.get("mySide")
                side_text = (
                    "你是红方"
                    if side == "red"
                    else "你是蓝方"
                    if side == "blue"
                    else "非参赛者"
                )
                lines.append(
                    f'  #{matchup.get("id")} {player1.get("username")}（红） vs '
                    f'{player2.get("username")}（蓝） | {side_text} | {matchup.get("status")}'
                )
            lines.append("用法：/book <房间ID> <对战ID>")
            return "\n".join(lines)

        player1 = data.get("player1") or {}
        player2 = data.get("player2") or {}
        side = data.get("mySide")
        side_text = "红方" if side == "red" else "蓝方" if side == "blue" else "-"
        room = data.get("room") or {}
        return (
            "预约成功，已通知对手确认。\n"
            f'对战 #{data.get("matchupId")}：{player1.get("username")}（红） vs '
            f'{player2.get("username")}（蓝）\n'
            f'你是{side_text}；房间：{room.get("roomName")}（{room.get("matchTime")}，UTC+8）\n'
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
class ListSchedulesTool(FunctionTool):
    name: str = "astracup_list_schedules"
    description: str = (
        "管理员查看最近赛程及赛程 ID，用于录入比分。非管理员会返回 403。"
        "用户说「给我录个比分」「查一下赛程 ID」时先调用它。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "可选，逗号分隔：pending,confirmed,completed,cancelled",
                },
                "season": {
                    "type": "string",
                    "description": "可选赛季 ID，例如 s1 / s2",
                },
                "limit": {
                    "type": "integer",
                    "description": "返回条数，默认 20，最大 100",
                },
            },
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        status: str = "",
        season: str = "",
        limit: int = 20,
    ) -> str:
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        try:
            limit = int(limit or 20)
        except (TypeError, ValueError):
            limit = 20
        return _dump(
            await self.client.schedules(
                platform,
                platform_user_id,
                status=(status or "").strip() or None,
                season=(season or "").strip() or None,
                limit=limit,
            )
        )


@dataclass
class RecordScoreTool(FunctionTool):
    name: str = "astracup_record_score"
    description: str = (
        "管理员记录比赛比分，写入后端 match_schedules。调用前必须确认赛程 ID 和双方比分；"
        "非管理员会返回 403。红方=player1，蓝方=player2。"
    )
    parameters: dict = field(
        default_factory=lambda: {
            "type": "object",
            "properties": {
                "schedule_id": {
                    "type": "integer",
                    "description": "赛程 ID",
                },
                "red_score": {
                    "type": "integer",
                    "description": "红方（player1）比分，0-99",
                },
                "blue_score": {
                    "type": "integer",
                    "description": "蓝方（player2）比分，0-99",
                },
                "status": {
                    "type": "string",
                    "description": "可选状态：pending/confirmed/completed/cancelled，默认 completed",
                },
                "season": {
                    "type": "string",
                    "description": "可选赛季 ID；不传则保留赛程原赛季",
                },
            },
            "required": ["schedule_id", "red_score", "blue_score"],
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        schedule_id: int,
        red_score: int,
        blue_score: int,
        status: str = "",
        season: str = "",
    ) -> str:
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        return _dump(
            await self.client.record_score(
                platform=platform,
                platform_user_id=platform_user_id,
                schedule_id=int(schedule_id),
                red_score=int(red_score),
                blue_score=int(blue_score),
                status=(status or "").strip() or None,
                season=(season or "").strip() or None,
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
        "以当前消息发送者绑定的 AstarCup 身份预约房间。必须先调用 astracup_get_player "
        "查看对战安排，确认 matchup_id、红蓝方（红=player1，蓝=player2）和参赛者身份后再预约。"
        "没有 matchup_id 时不要调用本工具。"
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
                    "description": "对战 ID，必填；来自 astracup_get_player 的 matchups/availableMatchups",
                },
            },
            "required": ["room_id", "matchup_id"],
        }
    )
    client: AstracupClient = None  # type: ignore[assignment]

    async def run(
        self,
        event: AstrMessageEvent,
        room_id: int,
        matchup_id: int,
    ) -> str:
        if not matchup_id:
            return _dump(
                {
                    "ok": False,
                    "error": "缺少 matchup_id；请先查看对战安排并确认对战 ID",
                }
            )
        platform, platform_user_id = _platform_identity(event)
        if not platform_user_id:
            return _dump({"ok": False, "error": "无法获取发送者的平台账号 ID"})
        return _dump(
            await self.client.book(
                platform=platform,
                platform_user_id=platform_user_id,
                room_id=room_id,
                matchup_id=matchup_id,
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
