use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSettings {
    pub require_approval: bool,
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            require_approval: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub name: String,
}

pub struct User {
    pub info: UserInfo,
    pub tx: mpsc::UnboundedSender<axum::extract::ws::Message>,
}

pub struct Room {
    pub code: String,
    pub owner_id: Uuid,
    pub members: Vec<User>,
    pub pending_joins: Vec<PendingJoin>,
    pub settings: RoomSettings,
}

pub struct PendingJoin {
    pub user_info: UserInfo,
    pub tx: mpsc::UnboundedSender<axum::extract::ws::Message>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum RoomMessage {
    Joined { user: UserInfo, members: Vec<UserInfo>, is_owner: bool },
    Left { user_id: Uuid, new_owner_id: Option<Uuid> },
    JoinRequest { user: UserInfo },
    JoinResponse { approved: bool, code: String },
    Chat { user_id: Uuid, message: String }, // Future feature placeholder
    Error { message: String },
}
