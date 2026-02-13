use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    dto::tournament_settings::{
        CreateTournamentSettingRequest, TournamentSettingQueryParams,
        UpdateTournamentSettingRequest,
    },
    entity::tournament_settings,
    error::{AppError, Result},
    service::ServiceState,
};

pub struct TournamentSettingService {
    db: DatabaseConnection,
    current_settings_cache: Arc<RwLock<Option<tournament_settings::Model>>>,
}

impl TournamentSettingService {
    pub fn new(state: ServiceState) -> Self {
        Self {
            db: state.db,
            current_settings_cache: state.tournament_settings_cache,
        }
    }

    pub async fn find_all(
        &self,
        params: TournamentSettingQueryParams,
    ) -> Result<(Vec<tournament_settings::Model>, u64)> {
        let mut query = tournament_settings::Entity::find();

        if let Some(current_season) = params.current_season {
            query =
                query.filter(tournament_settings::Column::CurrentSeason.eq(Some(current_season)));
        }

        query = query.order_by_desc(tournament_settings::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let tournament_settings = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((tournament_settings, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<tournament_settings::Model> {
        tournament_settings::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("比赛设置 {} 不存在", id)))
    }

    pub async fn create(
        &self,
        data: CreateTournamentSettingRequest,
    ) -> Result<tournament_settings::Model> {
        let now = chrono::Utc::now();

        let tournament_setting = tournament_settings::ActiveModel {
            tournament_name: Set(data.tournament_name),
            max_pp_for_registration: Set(data.max_pp_for_registration),
            min_pp_for_registration: Set(data.min_pp_for_registration),
            current_season: Set(data.current_season),
            current_season_stage: Set(data.current_season_stage),
            mappool_visible: Set(data.mappool_visible),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let result = tournament_setting.insert(&self.db).await?;

        // 更新缓存
        let mut cache = self.current_settings_cache.write().await;
        *cache = Some(result.clone());

        Ok(result)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdateTournamentSettingRequest,
    ) -> Result<tournament_settings::Model> {
        let mut tournament_setting: tournament_settings::ActiveModel =
            self.find_by_id(id).await?.into();

        if let Some(tournament_name) = data.tournament_name {
            tournament_setting.tournament_name = Set(tournament_name);
        }

        if let Some(max_pp_for_registration) = data.max_pp_for_registration {
            tournament_setting.max_pp_for_registration = Set(Some(max_pp_for_registration));
        }

        if let Some(min_pp_for_registration) = data.min_pp_for_registration {
            tournament_setting.min_pp_for_registration = Set(Some(min_pp_for_registration));
        }

        if let Some(current_season) = data.current_season {
            tournament_setting.current_season = Set(Some(current_season));
        }

        if let Some(current_season_stage) = data.current_season_stage {
            tournament_setting.current_season_stage = Set(Some(current_season_stage));
        }

        if let Some(mappool_visible) = data.mappool_visible {
            tournament_setting.mappool_visible = Set(Some(mappool_visible));
        }

        tournament_setting.updated_at = Set(chrono::Utc::now());

        let result = tournament_setting.update(&self.db).await?;

        // 更新缓存
        let mut cache = self.current_settings_cache.write().await;
        *cache = Some(result.clone());

        Ok(result)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        // 检查缓存中是否有这个项目
        {
            let cache = self.current_settings_cache.read().await;
            if let Some(cached_settings) = cache.as_ref() {
                if cached_settings.id == id {
                    // 删除的是缓存的项目，清除缓存
                    drop(cache); // 释放读锁
                    let mut cache = self.current_settings_cache.write().await;
                    *cache = None;
                }
            }
        }

        let result = tournament_settings::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("比赛设置 {} 不存在", id)));
        }
        Ok(())
    }

    pub async fn get_current_settings(&self) -> Result<Option<tournament_settings::Model>> {
        Ok(tournament_settings::Entity::find()
            .order_by_desc(tournament_settings::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    pub async fn get_current_settings_cached(&self) -> Result<Option<tournament_settings::Model>> {
        // 首先尝试从缓存读取
        {
            let cache = self.current_settings_cache.read().await;
            if let Some(settings) = cache.as_ref() {
                return Ok(Some(settings.clone()));
            }
        }

        // 缓存未命中，从数据库读取
        let settings = tournament_settings::Entity::find()
            .order_by_desc(tournament_settings::Column::CreatedAt)
            .one(&self.db)
            .await?;

        // 更新缓存
        if let Some(ref settings) = settings {
            let mut cache = self.current_settings_cache.write().await;
            *cache = Some(settings.clone());
        }

        Ok(settings)
    }

    pub async fn invalidate_cache(&self) {
        let mut cache = self.current_settings_cache.write().await;
        *cache = None;
    }
}
