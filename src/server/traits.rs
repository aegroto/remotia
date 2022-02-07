use super::types::ServerFrameData;

use async_trait::async_trait;

#[async_trait]
pub trait FrameProcessor {
    async fn process(&mut self, frame_data: ServerFrameData) -> ServerFrameData;
}