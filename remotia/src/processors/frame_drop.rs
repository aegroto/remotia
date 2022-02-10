use crate::{traits::FrameProcessor, types::FrameData, error::DropReason};
use async_trait::async_trait;
use log::debug;

pub struct TimestampDiffBasedFrameDropper {
    threshold: u128,
    stat_id: String,
}

impl TimestampDiffBasedFrameDropper {
    pub fn new(stat_id: &str, threshold: u128) -> Self {
        Self {
            threshold,
            stat_id: stat_id.to_string(),
        }
    }
}

#[async_trait]
impl FrameProcessor for TimestampDiffBasedFrameDropper {
    async fn process(&mut self, mut frame_data: FrameData) -> Option<FrameData> {
        let diff_value = frame_data.get(&self.stat_id);

        if diff_value > self.threshold {
            frame_data.set_drop_reason(Some(DropReason::StaleFrame));
        }

        Some(frame_data)
    }
}
