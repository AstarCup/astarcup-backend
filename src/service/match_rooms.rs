use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::match_rooms::{CreateMatchRoomRequest, MatchRoomQueryParams, UpdateMatchRoomRequest},
    entity::{match_rooms, sea_orm_active_enums::Status},
    error::{AppError, Result},
};

pub struct MatchRoomService {
    db: DatabaseConnection,
}

impl MatchRoomService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: MatchRoomQueryParams,
    ) -> Result<(Vec<match_rooms::Model>, u64)> {
        let mut query = match_rooms::Entity::find();

        if let Some(round_number) = params.round_number {
            query = query.filter(match_rooms::Column::RoundNumber.eq(round_number));
        }

        if let Some(status) = params.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            query = query.filter(match_rooms::Column::Status.eq(Some(status_enum)));
        }

        query = query.order_by_asc(match_rooms::Column::Id);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let match_rooms = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((match_rooms, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<match_rooms::Model> {
        match_rooms::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("匹配房间 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreateMatchRoomRequest) -> Result<match_rooms::Model> {
        let now = chrono::Utc::now();

        let match_room = match_rooms::ActiveModel {
            room_name: Set(data.room_name),
            round_number: Set(data.round_number),
            match_date: Set(data.match_date),
            match_time: Set(data.match_time),
            match_number: Set(data.match_number),
            max_participants: Set(data.max_participants),
            status: Set(Some(Status::Available)),
            created_by: Set(data.created_by),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(match_room.insert(&self.db).await?)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdateMatchRoomRequest,
    ) -> Result<match_rooms::Model> {
        let mut match_room: match_rooms::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(room_name) = data.room_name {
            match_room.room_name = Set(room_name);
        }

        if let Some(round_number) = data.round_number {
            match_room.round_number = Set(round_number);
        }

        if let Some(match_date) = data.match_date {
            match_room.match_date = Set(match_date);
        }

        if let Some(match_time) = data.match_time {
            match_room.match_time = Set(match_time);
        }

        if let Some(match_number) = data.match_number {
            match_room.match_number = Set(match_number);
        }

        if let Some(max_participants) = data.max_participants {
            match_room.max_participants = Set(Some(max_participants));
        }

        if let Some(status) = data.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            match_room.status = Set(Some(status_enum));
        }

        match_room.updated_at = Set(chrono::Utc::now());

        Ok(match_room.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = match_rooms::Entity::delete_by_id(id).exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("匹配房间 {} 不存在", id)));
        }
        Ok(())
    }
}
