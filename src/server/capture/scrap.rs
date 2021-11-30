use scrap::{Capturer, Display, Frame};

use core::slice;
use std::io::ErrorKind::WouldBlock;

use super::FrameCapturer;

pub struct ScrapFrameCapturer {
    capturer: Capturer,
}

impl ScrapFrameCapturer {
    pub fn new(capturer: Capturer) -> Self {
        Self { capturer }
    }

    pub fn new_from_primary() -> Self {
        let display = Display::primary().expect("Couldn't find primary display.");
        let capturer = Capturer::new(display).expect("Couldn't begin capture.");
        Self { capturer }
    }
}

impl FrameCapturer for ScrapFrameCapturer {
    fn capture(&mut self) -> Result<&[u8], std::io::Error> {
        match self.capturer.frame() {
            Ok(buffer) => {
                let frame_slice = unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len()) };
                return Ok(frame_slice);
            }
            Err(error) => {
                if error.kind() == WouldBlock {
                    return Err(error);
                } else {
                    panic!("Error: {}", error);
                }
            }
        }
    }

    fn width(&self) -> usize {
        self.capturer.width()
    }

    fn height(&self) -> usize {
        self.capturer.height()
    }
}
