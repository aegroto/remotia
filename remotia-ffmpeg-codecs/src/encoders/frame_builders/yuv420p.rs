use std::time::Instant;

use log::debug;
use remotia::server::utils::bgr2yuv::{raster};
use rsmpeg::{avcodec::AVCodecContext, avutil::AVFrame};

pub struct YUV420PAVFrameBuilder {
    frame_count: i64,
}

impl YUV420PAVFrameBuilder {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
        }
    }

    pub fn create_avframe(
        &mut self,
        encode_context: &mut AVCodecContext,
        frame_buffer: &[u8],
        key_frame: bool
    ) -> AVFrame {
        let mut avframe = AVFrame::new();
        avframe.set_format(encode_context.pix_fmt);
        avframe.set_width(encode_context.width);
        avframe.set_height(encode_context.height);
        avframe.set_pts(self.frame_count);
        if key_frame {
            avframe.set_pict_type(1);
        }
        avframe.alloc_buffer().unwrap();

        let data = avframe.data;
        let linesize = avframe.linesize;
        // let width = encode_context.width as usize;
        let height = encode_context.height as usize;

        let linesize_y = linesize[0] as usize;
        let linesize_cb = linesize[1] as usize;
        let linesize_cr = linesize[2] as usize;
        let mut y_data = unsafe { std::slice::from_raw_parts_mut(data[0], height * linesize_y) };
        let mut cb_data = unsafe { std::slice::from_raw_parts_mut(data[1], height / 2 * linesize_cb) };
        let mut cr_data = unsafe { std::slice::from_raw_parts_mut(data[2], height / 2 * linesize_cr) };

        cb_data.fill(0);
        cr_data.fill(0);

        let start_time = Instant::now();
        raster::bgra_to_yuv_separate(
            frame_buffer,
            &mut y_data,
            &mut cb_data,
            &mut cr_data,
        );
        debug!("Time: {}", start_time.elapsed().as_millis());

        debug!("Created avframe #{}", avframe.pts);

        self.frame_count += 1;

        avframe
    }
}
