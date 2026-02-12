use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::match_schedules::{
        CreateMatchScheduleRequest, MatchScheduleQueryParams, UpdateMatchScheduleRequest,
    },
    entity::{match_schedules, sea_orm_active_enums::Status},
    error::{AppError, Result},
};

pub struct MatchScheduleService {
    db: DatabaseConnection,
}

impl MatchScheduleService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: MatchScheduleQueryParams,
    ) -> Result<(Vec<match_schedules::Model>, u64)> {
        let mut query = match_schedules::Entity::find();

        if let Some(room_id) = params.room_id {
            query = query.filter(match_schedules::Column::RoomId.eq(room_id));
        }

        if let Some(status) = params.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            query = query.filter(match_schedules::Column::Status.eq(Some(status_enum)));
        }

        if let Some(player_osu_id) = params.player_osu_id {
            query = query.filter(
                match_schedules::Column::Player1OsuId
                    .eq(&player_osu_id)
                    .or(match_schedules::Column::Player2OsuId.eq(&player_osu_id)),
            );
        }

        query = query.order_by_desc(match_schedules::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let match_schedules = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((match_schedules, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<match_schedules::Model> {
        match_schedules::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("匹配赛程 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreateMatchScheduleRequest) -> Result<match_schedules::Model> {
        let now = chrono::Utc::now();

        let match_schedule = match_schedules::ActiveModel {
            room_id: Set(data.room_id),
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

        Ok(match_schedule.insert(&self.db).await?)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdateMatchScheduleRequest,
    ) -> Result<match_schedules::Model> {
        let mut match_schedule: match_schedules::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(red_score) = data.red_score {
            match_schedule.red_score = Set(Some(red_score));
        }

        if let Some(blue_score) = data.blue_score {
            match_schedule.blue_score = Set(Some(blue_score));
        }

        if let Some(status) = data.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            match_schedule.status = Set(Some(status_enum));
        }

        if let Some(replay_link) = data.replay_link {
            match_schedule.replay_link = Set(Some(replay_link));
        }

        if let Some(match_link) = data.match_link {
            match_schedule.match_link = Set(Some(match_link));
        }

        match_schedule.updated_at = Set(chrono::Utc::now());

        Ok(match_schedule.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = match_schedules::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("匹配赛程 {} 不存在", id)));
        }
        Ok(())
    }
}
