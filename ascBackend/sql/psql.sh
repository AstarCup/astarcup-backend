#!/bin/sh
# 用 .env 里的 DATABASE_URL 执行 psql
# 用法（在 ascBackend/ 目录下）：sh sql/psql.sh -c "SELECT count(*) FROM news"
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

psql "$DB_URL" "$@"
