-- 工作人员名单（「联系我们」公开展示）：只记录谁是 staff、分类与职位；
-- 姓名/头像/封面等用户资料统一从 "User" 表按 osuId 关联获取。
-- 幂等，可重复执行。

CREATE TABLE IF NOT EXISTS staff_members (
    id SERIAL PRIMARY KEY,
    osu_id TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL DEFAULT '',
    position TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 确保名单里的人都有 "User" 行（已在库的自动跳过）：
-- 用户资料由登录/cover 刷新任务从 osu! 自动补全与保鲜。
INSERT INTO "User" ("osuId", "username", "registeredAt", "country", "updatedAt")
VALUES
    ('6781503', 'AeCw', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('17898289', 'ShikaNai', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('25767690', 'Yoizuki', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('17823832', 'Kieost', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('30826405', 'bobbycyl', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('30973609', 'MoeRain233', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('5936405', '-xxx Rumia-', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('32657919', 'fuyukiS', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('30284920', 'Bochi the osu', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('3046750', 'oliwakami', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('16134378', 'Akisa 18', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('9037287', 'Zh_Jk', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('6149313', 'Pata-Mon', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('13999223', '[SHK]T_T', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('13184288', 'Dana Iclucia', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('6338477', 'yutro', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('11511458', 'Rinne_0', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP),
    ('1646397', 'Crystal', CURRENT_TIMESTAMP, '', CURRENT_TIMESTAMP)
ON CONFLICT ("osuId") DO NOTHING;

-- 名单种子（category ← 原 role，position ← 原 description）；表非空时不重复灌入
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM staff_members) THEN
        INSERT INTO staff_members (osu_id, category, position, sort_order) VALUES
            ('6781503', '主办 视觉设计', '主办 视觉设计', 1),
            ('17898289', '裁判', '裁判', 2),
            ('25767690', '裁判 解说', '比赛裁判 解说', 3),
            ('17823832', '定制图mapool', '定制图', 4),
            ('30826405', '图池策划', '图池策划', 5),
            ('30973609', '客户端定制', '大神', 6),
            ('5936405', '图池策划', '图池策划', 7),
            ('32657919', '图池策划 直播 测图', '图池策划 直播 测图', 8),
            ('30284920', '图池测试', '测试比赛图池 图池策划', 9),
            ('3046750', '图池测试', '测试比赛图池', 10),
            ('16134378', '图池策划', '图池策划', 11),
            ('9037287', 'jk', '这是zh_jk', 12),
            ('6149313', 'special', '规则校验', 13),
            ('13999223', 'special', '比赛宣传 男人中的男人，帝王中的帝王', 14),
            ('13184288', 'special', '比赛宣传', 15),
            ('6338477', 'special', 'Replay外援', 16),
            ('11511458', '图池策划、测试', '图池策划 测试', 17),
            ('1646397', '图池测试', '图池测试', 18);
    END IF;
END $$;
