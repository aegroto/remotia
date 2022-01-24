use beryllium::gl_window::GlWindow;
use log::debug;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};

use crate::{client::utils::decoding::packed_bgr_to_packed_rgba, common::window::create_gl_window};

use super::Renderer;

pub struct BerylliumRenderer {
    _gl_win: GlWindow,
    pixels: Pixels,

    canvas_width: u32,
    canvas_height: u32,
}
unsafe impl Send for BerylliumRenderer {}

impl BerylliumRenderer {
    pub fn new(canvas_width: u32, canvas_height: u32) -> Self {
        // Init display
        let gl_win = create_gl_window(canvas_width as i32, canvas_height as i32);
        let window = &*gl_win;

        let pixels = {
            let surface_texture = SurfaceTexture::new(canvas_width, canvas_height, &window);
            PixelsBuilder::new(canvas_width, canvas_height, surface_texture)
                .build()
                .unwrap()
        };

        Self {
            _gl_win: gl_win,
            pixels,
            canvas_width,
            canvas_height,
        }
    }
}

impl Renderer for BerylliumRenderer {
    fn render(&mut self, raw_frame_buffer: &[u8]) {
        packed_bgr_to_packed_rgba(&raw_frame_buffer, self.pixels.get_frame());
        self.pixels.render().unwrap();
    }

    fn handle_feedback(&mut self, message: crate::common::feedback::FeedbackMessage) {
        debug!("Feedback message: {:?}", message);
    }

    fn get_buffer_size(&self) -> usize {
        (self.canvas_width * self.canvas_height * 3) as usize
    }
}
