use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::messages::{CreateMessageRequest, MessageQueryParams, UpdateMessageRequest},
    entity::{
        messages,
        sea_orm_active_enums::{Status, Type},
    },
    error::{AppError, Result},
};

pub struct MessageService {
    db: DatabaseConnection,
}

impl MessageService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: MessageQueryParams,
    ) -> Result<(Vec<messages::Model>, u64)> {
        let mut query = messages::Entity::find();

        if let Some(sender_osu_id) = params.sender_osu_id {
            query = query.filter(messages::Column::SenderOsuId.eq(sender_osu_id));
        }

        if let Some(receiver_osu_id) = params.receiver_osu_id {
            query = query.filter(messages::Column::ReceiverOsuId.eq(receiver_osu_id));
        }

        if let Some(status) = params.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            query = query.filter(messages::Column::Status.eq(Some(status_enum)));
        }

        query = query.order_by_desc(messages::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let messages = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((messages, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<messages::Model> {
        messages::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("消息 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreateMessageRequest) -> Result<messages::Model> {
        let now = chrono::Utc::now();

        let message_type = data.r#type.map(|t| match t.as_str() {
            "system" => Type::System,
            "match_invitation" => Type::MatchInvitation,
            "match_response" => Type::MatchResponse,
            _ => panic!("无效的消息类型"),
        });

        let message = messages::ActiveModel {
            sender_osu_id: Set(data.sender_osu_id),
            sender_username: Set(data.sender_username),
            receiver_osu_id: Set(data.receiver_osu_id),
            receiver_username: Set(data.receiver_username),
            r#type: Set(message_type),
            title: Set(data.title),
            content: Set(data.content),
            related_matchup_id: Set(data.related_matchup_id),
            status: Set(Some(Status::Available)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(message.insert(&self.db).await?)
    }

    pub async fn update(&self, id: i32, data: UpdateMessageRequest) -> Result<messages::Model> {
        let mut message: messages::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(title) = data.title {
            message.title = Set(title);
        }

        if let Some(content) = data.content {
            message.content = Set(content);
        }

        if let Some(status) = data.status {
            let status_enum = match status.as_str() {
                "available" => Status::Available,
                "scheduled" => Status::Scheduled,
                "completed" => Status::Completed,
                _ => return Err(AppError::ValidationError("无效的状态".into())),
            };
            message.status = Set(Some(status_enum));
            if status == "completed" {
                message.response_time = Set(Some(chrono::Utc::now()));
            }
        }

        if let Some(response_action) = data.response_action {
            message.response_action = Set(Some(response_action));
        }

        message.updated_at = Set(chrono::Utc::now());

        Ok(message.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = messages::Entity::delete_by_id(id).exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("消息 {} 不存在", id)));
        }
        Ok(())
    }
}
