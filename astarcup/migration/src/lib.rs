pub use sea_orm_migration::prelude::*;

mod m20260211_140546_user;
mod m20260211_140547_map_selections;
mod m20260211_140548_match_rooms;
mod m20260211_140549_match_schedules;
mod m20260211_140550_player_matchups;
mod m20260211_140551_messages;
mod m20260211_140552_map_comments;
mod m20260211_140553_tournament_settings;
mod m20260213_094956_add_refresh_tokens;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260211_140546_user::Migration),
            Box::new(m20260211_140547_map_selections::Migration),
            Box::new(m20260211_140548_match_rooms::Migration),
            Box::new(m20260211_140549_match_schedules::Migration),
            Box::new(m20260211_140550_player_matchups::Migration),
            Box::new(m20260211_140551_messages::Migration),
            Box::new(m20260211_140552_map_comments::Migration),
            Box::new(m20260211_140553_tournament_settings::Migration),
            Box::new(m20260213_094956_add_refresh_tokens::Migration),
        ]
    }
}
