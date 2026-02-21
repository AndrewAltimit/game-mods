//! Frame scaling and pixel format conversion.

use crate::error::{VideoError, VideoResult};
use crate::{BYTES_PER_PIXEL, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling::{Context as SwsContext, Flags};
use ffmpeg_next::util::frame::video::Video as VideoFrame;

/// Handles scaling and converting video frames to RGBA at a target resolution.
pub struct FrameScaler {
    context: Option<SwsContext>,
    output_width: u32,
    output_height: u32,
    /// Reusable output buffer to avoid allocations.
    output_buffer: Vec<u8>,
    /// Reusable output frame.
    output_frame: VideoFrame,
    /// Whether the output frame buffer has been allocated.
    frame_allocated: bool,
    /// Last input dimensions/format for detecting changes.
    last_input_width: u32,
    last_input_height: u32,
    last_input_format: Pixel,
}

impl FrameScaler {
    /// Create a new scaler with the default 720p output resolution.
    pub fn new() -> VideoResult<Self> {
        Self::with_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }

    /// Create a new scaler with a custom output resolution.
    pub fn with_size(width: u32, height: u32) -> VideoResult<Self> {
        let buffer_size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|s| s.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| {
                VideoError::InvalidFrame(format!(
                    "frame size overflow: {width}x{height}x{BYTES_PER_PIXEL}"
                ))
            })?;
        let mut output_frame = VideoFrame::empty();
        output_frame.set_format(Pixel::RGBA);
        output_frame.set_width(width);
        output_frame.set_height(height);

        Ok(Self {
            context: None,
            output_width: width,
            output_height: height,
            output_buffer: vec![0u8; buffer_size],
            output_frame,
            frame_allocated: false,
            last_input_width: 0,
            last_input_height: 0,
            last_input_format: Pixel::None,
        })
    }

    /// Get the output width.
    pub fn width(&self) -> u32 {
        self.output_width
    }

    /// Get the output height.
    pub fn height(&self) -> u32 {
        self.output_height
    }

    /// Scale and convert a frame to RGBA.
    ///
    /// Returns a reference to the internal buffer containing the RGBA data.
    /// The buffer is valid until the next call to `scale`.
    pub fn scale(&mut self, input: &VideoFrame) -> VideoResult<&[u8]> {
        let input_format = input.format();
        let input_width = input.width();
        let input_height = input.height();

        // Create or recreate the scaling context if input parameters changed
        let needs_new_context = self.context.is_some()
            && (self.last_input_width != input_width
                || self.last_input_height != input_height
                || self.last_input_format != input_format);

        if needs_new_context || self.context.is_none() {
            self.context = Some(
                SwsContext::get(
                    input_format,
                    input_width,
                    input_height,
                    Pixel::RGBA,
                    self.output_width,
                    self.output_height,
                    Flags::BILINEAR,
                )
                .map_err(|e| VideoError::ScaleError(e.to_string()))?,
            );
            self.last_input_width = input_width;
            self.last_input_height = input_height;
            self.last_input_format = input_format;
        }

        let ctx = self
            .context
            .as_mut()
            .ok_or_else(|| VideoError::ScaleError("scaling context not initialized".to_string()))?;

        // Allocate output frame buffer if needed
        if !self.frame_allocated {
            // SAFETY: `output_frame` is a valid AVFrame with format, width, and height
            // set. `av_frame_get_buffer` allocates data planes for these parameters.
            unsafe {
                let ret = ffmpeg_next::ffi::av_frame_get_buffer(
                    self.output_frame.as_mut_ptr(),
                    32, // alignment
                );
                if ret < 0 {
                    return Err(VideoError::ScaleError(
                        "failed to allocate output frame buffer".to_string(),
                    ));
                }
            }
            self.frame_allocated = true;
        }

        // Run the scaling operation
        ctx.run(input, &mut self.output_frame)
            .map_err(|e| VideoError::ScaleError(e.to_string()))?;

        // Copy the frame data to our output buffer
        // RGBA frames have a single plane
        let data = self.output_frame.data(0);
        let linesize = self.output_frame.stride(0);
        let row_bytes = (self.output_width as usize) * BYTES_PER_PIXEL;

        // Handle potential padding in frame rows
        if linesize == row_bytes {
            // No padding, direct copy
            let copy_size = row_bytes * (self.output_height as usize);
            self.output_buffer[..copy_size].copy_from_slice(&data[..copy_size]);
        } else {
            // Row-by-row copy to handle padding
            for y in 0..self.output_height as usize {
                let src_offset = y * linesize;
                let dst_offset = y * row_bytes;
                self.output_buffer[dst_offset..dst_offset + row_bytes]
                    .copy_from_slice(&data[src_offset..src_offset + row_bytes]);
            }
        }

        Ok(&self.output_buffer)
    }

    /// Get the expected output buffer size in bytes.
    ///
    /// Panics on overflow (dimensions are validated in `with_size`).
    pub fn buffer_size(&self) -> usize {
        (self.output_width as usize)
            .checked_mul(self.output_height as usize)
            .and_then(|s| s.checked_mul(BYTES_PER_PIXEL))
            .expect("buffer_size overflow (dimensions should have been validated)")
    }
}

impl Default for FrameScaler {
    fn default() -> Self {
        // Default 720p dimensions are always valid; unwrap is safe
        Self::new().expect("default 720p scaler dimensions should never overflow")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaler_dimensions() {
        let scaler = FrameScaler::new().unwrap();
        assert_eq!(scaler.width(), DEFAULT_WIDTH);
        assert_eq!(scaler.height(), DEFAULT_HEIGHT);
        assert_eq!(scaler.buffer_size(), 1280 * 720 * 4);
    }

    #[test]
    fn test_custom_dimensions() {
        let scaler = FrameScaler::with_size(640, 480).unwrap();
        assert_eq!(scaler.width(), 640);
        assert_eq!(scaler.height(), 480);
        assert_eq!(scaler.buffer_size(), 640 * 480 * 4);
    }

    #[test]
    fn test_overflow_dimensions() {
        let result = FrameScaler::with_size(u32::MAX, u32::MAX);
        assert!(result.is_err());
    }
}
