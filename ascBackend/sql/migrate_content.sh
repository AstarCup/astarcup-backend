#!/bin/sh
# 应用内容迁移 sql/content_tables.sql（news + season_rules，幂等可重复执行）
#
# 用法（在 ascBackend/ 目录下）：
#   sh sql/migrate_content.sh                        # 读取 ./.env 里的 DATABASE_URL
#   DATABASE_URL='postgres://...' sh sql/migrate_content.sh   # 指定其它库（如线上）
set -e

if [ -z "$DATABASE_URL" ] && [ -f .env ]; then
  DB_URL=$(grep '^DATABASE_URL=' .env | cut -d= -f2-)
else
  DB_URL="$DATABASE_URL"
fi

if [ -z "$DB_URL" ]; then
  echo "错误：未找到 DATABASE_URL（环境变量或 .env）" >&2
  exit 1
fi

psql "$DB_URL" -v ON_ERROR_STOP=1 -f sql/content_tables.sql

echo "--- 校验 ---"
psql "$DB_URL" -c "SELECT count(*) AS news_count FROM news" \
               -c "SELECT season, length(content) AS content_len, updated_at FROM season_rules ORDER BY season"
