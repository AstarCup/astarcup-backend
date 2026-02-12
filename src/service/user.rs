use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::user::{CreateUserRequest, UpdateApprovedRequest, UpdateRegistrationStatusRequest, UpdateSeasonRequest, UpdateUserGroupRequest, UpdateUserRequest, UserQueryParams},
    entity::{sea_orm_active_enums::{RegistrationStatus, UserGroup}, user},
    error::{AppError, Result},
};

pub struct UserService {
    db: DatabaseConnection,
}

impl UserService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(&self, params: UserQueryParams) -> Result<(Vec<user::Model>, u64)> {
        let mut query = user::Entity::find();

        if let Some(username) = params.username {
            query = query.filter(user::Column::Username.contains(username));
        }

        if let Some(country) = params.country {
            query = query.filter(user::Column::Country.eq(country));
        }

        if let Some(user_group) = params.user_group {
            let group_enum = match user_group.as_str() {
                "player" => UserGroup::Player,
                "admin" => UserGroup::Admin,
                _ => return Err(AppError::ValidationError("无效的用户组".into())),
            };
            query = query.filter(user::Column::UserGroup.eq(group_enum));
        }

        query = query.order_by_asc(user::Column::Id);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let users = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((users, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<user::Model> {
        user::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("用户 {} 不存在", id)))
    }

    pub async fn find_by_osu_id(&self, osu_id: &str) -> Result<Option<user::Model>> {
        Ok(user::Entity::find()
            .filter(user::Column::OsuId.eq(osu_id))
            .one(&self.db)
            .await?)
    }

    pub async fn create(&self, osu_id: String, data: CreateUserRequest) -> Result<user::Model> {
        let now = chrono::Utc::now();

        let user = user::ActiveModel {
            osu_id: Set(osu_id),
            username: Set(data.username),
            registered_at: Set(now),
            avatar_url: Set(data.avatar_url),
            pp: Set(data.pp),
            global_rank: Set(data.global_rank),
            country: Set(data.country),
            country_rank: Set(data.country_rank),
            approved: Set(None),
            user_group: Set(UserGroup::Player),
            season: Set(None),
            registration_status: Set(Some(RegistrationStatus::Pending)),
            gu_server_user_id: Set(None),
            gu_server_username: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(user.insert(&self.db).await?)
    }

    pub async fn update(&self, id: i32, data: UpdateUserRequest) -> Result<user::Model> {
        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(username) = data.username {
            user.username = Set(username);
        }

        if let Some(avatar_url) = data.avatar_url {
            user.avatar_url = Set(Some(avatar_url));
        }

        if let Some(pp) = data.pp {
            user.pp = Set(Some(pp));
        }

        if let Some(global_rank) = data.global_rank {
            user.global_rank = Set(Some(global_rank));
        }

        if let Some(country) = data.country {
            user.country = Set(Some(country));
        }

        if let Some(country_rank) = data.country_rank {
            user.country_rank = Set(Some(country_rank));
        }

        if let Some(approved) = data.approved {
            user.approved = Set(Some(approved));
        }

        if let Some(season) = data.season {
            user.season = Set(Some(season));
        }

        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }

    pub async fn update_user_group(
        &self,
        id: i32,
        data: UpdateUserGroupRequest,
    ) -> Result<user::Model> {
        let group_enum = match data.user_group.as_str() {
            "player" => UserGroup::Player,
            "admin" => UserGroup::Admin,
            _ => return Err(AppError::ValidationError("无效的用户组".into())),
        };

        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();
        user.user_group = Set(group_enum);
        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = user::Entity::delete_by_id(id).exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("用户 {} 不存在", id)));
        }
        Ok(())
    }

    pub async fn count_admins(&self) -> Result<u64> {
        Ok(user::Entity::find()
            .filter(user::Column::UserGroup.eq(UserGroup::Admin))
            .count(&self.db)
            .await?)
    }

    pub async fn update_registration_status(&self, id: i32, data: UpdateRegistrationStatusRequest) -> Result<user::Model> {
        let status_enum = match data.registration_status.as_str() {
            "pending" => RegistrationStatus::Pending,
            "confirmed" => RegistrationStatus::Confirmed,
            "rejected" => RegistrationStatus::Rejected,
            _ => return Err(AppError::ValidationError("无效的注册状态".into())),
        };

        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();
        user.registration_status = Set(Some(status_enum));
        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }

    pub async fn update_season(&self, id: i32, data: UpdateSeasonRequest) -> Result<user::Model> {
        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();
        user.season = Set(Some(data.season));
        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }

    pub async fn update_approved(&self, id: i32, data: UpdateApprovedRequest, is_admin: bool) -> Result<user::Model> {
        if !is_admin {
            return Err(AppError::Forbidden("只有管理员可以更新审批状态".into()));
        }

        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();
        user.approved = Set(Some(data.approved));
        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }

    pub async fn update_season_from_settings(&self, id: i32, season: i32) -> Result<user::Model> {
        let mut user: user::ActiveModel = self.find_by_id(id).await?.into();
        user.season = Set(Some(season));
        user.updated_at = Set(chrono::Utc::now());

        Ok(user.update(&self.db).await?)
    }
}
