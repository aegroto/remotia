use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FeedbackMessage {
    HighFrameDelay(u128)
}
