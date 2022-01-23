use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerFeedbackMessage {
    HighFrameDelay(u128)
}
