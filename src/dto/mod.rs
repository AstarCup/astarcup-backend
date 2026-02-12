pub mod auth;
pub mod map_comments;
pub mod map_selections;
pub mod match_rooms;
pub mod match_schedules;
pub mod messages;
pub mod player_matchups;
pub mod tournament_settings;
pub mod user;

/// 通用响应结构
#[derive(Debug, serde::Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            error: None,
        }
    }

    pub fn success_with_message(data: T, message: &str) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: Some(message.to_string()),
            error: None,
        }
    }

    pub fn error(error: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: None,
            error: Some(error.to_string()),
        }
    }

    pub fn error_with_message(error: &str, message: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message.to_string()),
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ApiResponseSchema<T: utoipa::ToSchema> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub error: Option<String>,
}

/// 分页请求参数
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
}

#[derive(Debug, serde::Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, total: u64, page: u64, page_size: u64) -> Self {
        let total_pages = (total as f64 / page_size as f64).ceil() as u64;
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PaginatedResponseSchema<T: utoipa::ToSchema> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}
