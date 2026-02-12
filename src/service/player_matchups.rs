use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::player_matchups::{
        CreatePlayerMatchupRequest, PlayerMatchupQueryParams, UpdatePlayerMatchupRequest,
    },
    entity::{player_matchups, sea_orm_active_enums::Status},
    error::{AppError, Result},
};

pub struct PlayerMatchupService {
    db: DatabaseConnection,
}

impl PlayerMatchupService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: PlayerMatchupQueryParams,
    ) -> Result<(Vec<player_matchups::Model>, u64)> {
        let mut query = player_matchups::Entity::find();

        if let Some(status) = params.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            query = query.filter(player_matchups::Column::Status.eq(Some(status_enum)));
        }

        if let Some(player_osu_id) = params.player_osu_id {
            query = query.filter(
                player_matchups::Column::Player1OsuId
                    .eq(&player_osu_id)
                    .or(player_matchups::Column::Player2OsuId.eq(&player_osu_id)),
            );
        }

        query = query.order_by_desc(player_matchups::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let player_matchups = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((player_matchups, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<player_matchups::Model> {
        player_matchups::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("玩家对战 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreatePlayerMatchupRequest) -> Result<player_matchups::Model> {
        let now = chrono::Utc::now();

        let player_matchup = player_matchups::ActiveModel {
            player1_osu_id: Set(data.player1_osu_id),
            player1_username: Set(data.player1_username),
            player2_osu_id: Set(data.player2_osu_id),
            player2_username: Set(data.player2_username),
            status: Set(Some(Status::Available)),
            created_by: Set(data.created_by),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(player_matchup.insert(&self.db).await?)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdatePlayerMatchupRequest,
    ) -> Result<player_matchups::Model> {
        let mut player_matchup: player_matchups::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(status) = data.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            player_matchup.status = Set(Some(status_enum));
        }

        player_matchup.updated_at = Set(chrono::Utc::now());

        Ok(player_matchup.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = player_matchups::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("玩家对战 {} 不存在", id)));
        }
        Ok(())
    }
}
