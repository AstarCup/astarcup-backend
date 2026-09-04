use std::io::Write;

// diesel 2.3+ 的 derive 宏需显式导入
#[allow(unused_imports, deprecated)]
use diesel::{AsExpression, FromSqlRow, QueryId, SqlType};

// FromSql/ToSql 实现所需的类型
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql};

/// 为 PostgreSQL 原生枚举类型生成 diesel SqlType + Rust 枚举 + FromSql/ToSql。
macro_rules! pg_enum {
    ($name:ident, $ty:ident, $pg_name:literal, { $($variant:ident => $val:literal),+ $(,)? }) => {
        #[derive(Debug, SqlType, QueryId)]
        #[diesel(postgres_type(name = $pg_name))]
        pub struct $ty;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow, QueryId)]
        #[diesel(sql_type = $ty)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $( $name::$variant => $val, )+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $( $val => Some($name::$variant), )+ _ => None }
            }
        }

        impl FromSql<$ty, Pg> for $name {
            fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
                match std::str::from_utf8(bytes.as_bytes())? {
                    $( $val => Ok($name::$variant), )+
                    other => Err(format!("未知的枚举值 {}::{}", $pg_name, other).into()),
                }
            }
        }

        impl ToSql<$ty, Pg> for $name {
            fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
                out.write_all(self.as_str().as_bytes())?;
                Ok(IsNull::No)
            }
        }
    };
}

pg_enum!(UserGroup, UserGroupType, "UserGroup", { Player => "player", Admin => "admin" });

pg_enum!(RegistrationStatus, RegistrationStatusType, "RegistrationStatus", {
    NotRegistered => "not_registered",
    Registered => "registered",
    Approved => "approved",
});

pg_enum!(RoomStatus, RoomStatusType, "RoomStatus", {
    Open => "open", Full => "full", Closed => "closed",
});

pg_enum!(ScheduleStatus, ScheduleStatusType, "ScheduleStatus", {
    Pending => "pending", Confirmed => "confirmed",
    Completed => "completed", Cancelled => "cancelled",
});

pg_enum!(MatchupStatus, MatchupStatusType, "MatchupStatus", {
    Available => "available", Scheduled => "scheduled", Completed => "completed",
});

pg_enum!(MessageType, MessageTypeType, "MessageType", {
    MatchInvitation => "match_invitation",
    SystemNotification => "system_notification",
    AdminMessage => "admin_message",
});

pg_enum!(MessageStatus, MessageStatusType, "MessageStatus", {
    Unread => "unread", Read => "read", Archived => "archived",
});

pg_enum!(StaffRole, StaffRoleType, "StaffRole", {
    Referee => "referee", Commentator => "commentator", Streamer => "streamer",
});
