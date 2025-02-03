use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DlType {
    Juan,
    Hua,
    Fanwai,
    Current,
    JuanHuaFanwai,
    Author,
    Local,
    Upscale,
}
