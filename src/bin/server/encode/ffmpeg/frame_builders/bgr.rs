use log::debug;
use rsmpeg::{avcodec::AVCodecContext, avutil::AVFrame, error::RsmpegError};

use crate::encode::{Encoder};

pub struct BGRAVFrameBuilder {
    frame_count: i64,
}

impl BGRAVFrameBuilder {
    pub fn new() -> Self {
        Self { frame_count: 0 }
    }

    pub fn create_avframe(
        &mut self,
        encode_context: &mut AVCodecContext,
        bgr_frame_buffer: &[u8],
    ) -> AVFrame {
        let mut avframe = AVFrame::new();
        avframe.set_format(encode_context.pix_fmt);
        avframe.set_width(encode_context.width);
        avframe.set_height(encode_context.height);
        avframe.set_pts(self.frame_count);
        avframe.alloc_buffer().unwrap();

        let data = avframe.data;
        let linesize = avframe.linesize;
        // let width = encode_context.width as usize;
        let height = encode_context.height as usize;
        // let pixels_count = width * height;

        let linesize = linesize[0] as usize;
        let data = unsafe { std::slice::from_raw_parts_mut(data[0], height * linesize) };

        data.copy_from_slice(bgr_frame_buffer);

        debug!("Created avframe #{}", avframe.pts);

        self.frame_count += 1;

        avframe
    }
}
