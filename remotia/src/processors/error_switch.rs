use async_trait::async_trait;

use crate::{server::pipeline::ascode::{AscodePipeline, feeder::AscodePipelineFeeder}, traits::FrameProcessor, types::FrameData};

pub struct OnErrorSwitch {
    feeder: AscodePipelineFeeder
}

impl OnErrorSwitch {
    pub fn new(destination_pipeline: &AscodePipeline) -> Self {
        Self {
            feeder: destination_pipeline.get_feeder()
        }
    }
}

#[async_trait]
impl FrameProcessor for OnErrorSwitch {
    async fn process(&mut self, frame_data: FrameData) -> Option<FrameData> {
        if frame_data.get_drop_reason().is_some() {
            self.feeder.feed(frame_data);
            None
        } else {
            Some(frame_data)
        }
    }
}
