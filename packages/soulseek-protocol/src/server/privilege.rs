use crate::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PrivilegesGift {
    username: String,
    days: u32,
}
