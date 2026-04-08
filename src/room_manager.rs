use crate::models::room::{Room, RoomSettings, User, UserInfo, RoomMessage};
use dashmap::DashMap;
use uuid::Uuid;
use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use rand::distr::{Alphanumeric, SampleString};

pub struct RoomManager {
    rooms: DashMap<String, Room>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
        }
    }

    pub fn create_room(&self, owner_info: UserInfo, tx: mpsc::UnboundedSender<axum::extract::ws::Message>) -> String {
        let code = self.generate_code();
        let room = Room {
            code: code.clone(),
            owner_id: owner_info.id,
            members: vec![User { info: owner_info, tx }],
            pending_joins: Vec::new(),
            settings: RoomSettings::default(),
        };
        self.rooms.insert(code.clone(), room);
        code
    }

    pub fn join_room(&self, code: &str, user_info: UserInfo, tx: mpsc::UnboundedSender<axum::extract::ws::Message>) -> Result<()> {
        let mut room = self.rooms.get_mut(code).ok_or_else(|| anyhow!("Room not found"))?;
        
        let is_owner = room.members.is_empty();
        if is_owner {
            room.owner_id = user_info.id;
        }

        let new_user_info = user_info.clone();
        room.members.push(User { info: user_info, tx });

        let members_list: Vec<UserInfo> = room.members.iter().map(|m| m.info.clone()).collect();

        // Notify the new member
        let welcome_msg = RoomMessage::Joined {
            user: new_user_info.clone(),
            members: members_list.clone(),
            is_owner,
        };
        if let Ok(text) = serde_json::to_string(&welcome_msg) {
            let last_member = room.members.last().unwrap();
            let _ = last_member.tx.send(axum::extract::ws::Message::Text(text.into()));
        }

        // Notify others
        let join_msg = RoomMessage::Joined {
            user: new_user_info.clone(),
            members: members_list,
            is_owner,
        };
        self.broadcast_room_except(&room, &join_msg, new_user_info.id);

        Ok(())
    }

    pub fn leave_room(&self, code: &str, user_id: Uuid) {
        let mut remove_room = false;
        if let Some(mut room) = self.rooms.get_mut(code) {
            room.members.retain(|m| m.info.id != user_id);
            
            if room.members.is_empty() {
                remove_room = true;
            } else if room.owner_id == user_id {
                // Transfer ownership
                if let Some(new_owner) = room.members.first() {
                    let new_owner_id = new_owner.info.id;
                    room.owner_id = new_owner_id;
                    let msg = RoomMessage::Left { user_id, new_owner_id: Some(new_owner_id) };
                    self.broadcast_room(&room, &msg);
                }
            } else {
                let msg = RoomMessage::Left { user_id, new_owner_id: None };
                self.broadcast_room(&room, &msg);
            }
        }
        
        if remove_room {
            self.rooms.remove(code);
            tracing::info!("Room {} deleted as it became empty.", code);
        }
    }

    pub fn broadcast_room(&self, room: &Room, msg: &RoomMessage) {
        if let Ok(text) = serde_json::to_string(msg) {
            let ws_msg = axum::extract::ws::Message::Text(text.into());
            for member in &room.members {
                let _ = member.tx.send(ws_msg.clone());
            }
        }
    }

    pub fn broadcast_room_except(&self, room: &Room, msg: &RoomMessage, except_id: Uuid) {
        if let Ok(text) = serde_json::to_string(msg) {
            let ws_msg = axum::extract::ws::Message::Text(text.into());
            for member in &room.members {
                if member.info.id != except_id {
                    let _ = member.tx.send(ws_msg.clone());
                }
            }
        }
    }

    fn generate_code(&self) -> String {
        loop {
            let code = Alphanumeric.sample_string(&mut rand::rng(), 6).to_uppercase();
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }
}
