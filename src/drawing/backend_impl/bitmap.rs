use crate::drawing::backend::{BackendCoord, DrawingBackend, DrawingErrorKind};
use crate::style::{Color, RGBAColor};
use image::{ImageError, Rgb, RgbImage};

use std::path::Path;

#[cfg(feature = "gif")]
mod gif_support {
    use super::*;
    use gif::{Encoder as GifEncoder, Frame as GifFrame, Repeat, SetParameter};
    use std::fs::File;

    pub(super) struct GifFile {
        encoder: GifEncoder<File>,
        height: u32,
        width: u32,
        delay: u32,
    }

    impl GifFile {
        pub(super) fn new<T: AsRef<Path>>(
            path: T,
            dim: (u32, u32),
            delay: u32,
        ) -> Result<Self, ImageError> {
            let mut encoder = GifEncoder::new(
                File::create(path.as_ref()).map_err(ImageError::IoError)?,
                dim.0 as u16,
                dim.1 as u16,
                &[],
            )?;

            encoder.set(Repeat::Infinite)?;

            Ok(Self {
                encoder,
                width: dim.0,
                height: dim.1,
                delay: (delay + 5) / 10,
            })
        }

        pub(super) fn flush_frame(&mut self, img: &mut RgbImage) -> Result<(), ImageError> {
            let mut new_img = RgbImage::new(self.width, self.height);
            std::mem::swap(&mut new_img, img);

            let mut frame = GifFrame::from_rgb_speed(
                self.width as u16,
                self.height as u16,
                &new_img.into_raw(),
                10,
            );

            frame.delay = self.delay as u16;

            self.encoder.write_frame(&frame)?;

            Ok(())
        }
    }
}

enum Target<'a> {
    File(&'a Path),
    Buffer(&'a mut Vec<u8>),
    #[cfg(feature = "gif")]
    Gif(Box<gif_support::GifFile>),
}

/// The backend that drawing a bitmap
pub struct BitMapBackend<'a> {
    /// The path to the image
    target: Target<'a>,
    /// The image object
    img: RgbImage,
    /// Flag indicates if the bitmap has been saved
    saved: bool,
}

impl<'a> BitMapBackend<'a> {
    /// Create a new bitmap backend
    pub fn new<T: AsRef<Path> + ?Sized>(path: &'a T, dimension: (u32, u32)) -> Self {
        Self {
            target: Target::File(path.as_ref()),
            img: RgbImage::new(dimension.0, dimension.1),
            saved: false,
        }
    }

    /// Create a new bitmap backend that generate GIF animation
    ///
    /// When this is used, the bitmap backend acts similar to a realtime rendering backend.
    /// When the program finished drawing one frame, use `present` function to flush the frame
    /// into the GIF file.
    ///
    /// - `path`: The path to the GIF file to create
    /// - `dimension`: The size of the GIF image
    /// - `speed`: The amount of time for each frame to display
    #[cfg(feature = "gif")]
    pub fn gif<T: AsRef<Path>>(
        path: T,
        dimension: (u32, u32),
        frame_delay: u32,
    ) -> Result<Self, ImageError> {
        Ok(Self {
            target: Target::Gif(Box::new(gif_support::GifFile::new(
                path,
                dimension,
                frame_delay,
            )?)),
            img: RgbImage::new(dimension.0, dimension.1),
            saved: false,
        })
    }

    /// Create a new bitmap backend which only lives in-memory
    pub fn with_buffer(buf: &'a mut Vec<u8>, dimension: (u32, u32)) -> Self {
        Self {
            target: Target::Buffer(buf),
            img: RgbImage::new(dimension.0, dimension.1),
            saved: false,
        }
    }
}

impl<'a> DrawingBackend for BitMapBackend<'a> {
    type ErrorType = ImageError;

    fn get_size(&self) -> (u32, u32) {
        (self.img.width(), self.img.height())
    }

    fn ensure_prepared(&mut self) -> Result<(), DrawingErrorKind<ImageError>> {
        self.saved = false;
        Ok(())
    }

    fn present(&mut self) -> Result<(), DrawingErrorKind<ImageError>> {
        match &mut self.target {
            Target::File(path) => {
                self.img
                    .save(&path)
                    .map_err(|x| DrawingErrorKind::DrawingError(ImageError::IoError(x)))?;
                self.saved = true;
                Ok(())
            }
            Target::Buffer(target) => {
                let mut actual_img = RgbImage::new(1, 1);
                std::mem::swap(&mut actual_img, &mut self.img);
                target.clear();
                target.append(&mut actual_img.into_raw());
                Ok(())
            }
            #[cfg(feature = "gif")]
            Target::Gif(target) => {
                target
                    .flush_frame(&mut self.img)
                    .map_err(DrawingErrorKind::DrawingError)?;
                self.saved = true;
                Ok(())
            }
        }
    }

    fn draw_pixel(
        &mut self,
        point: BackendCoord,
        color: &RGBAColor,
    ) -> Result<(), DrawingErrorKind<ImageError>> {
        if point.0 as u32 >= self.img.width()
            || point.0 < 0
            || point.1 as u32 >= self.img.height()
            || point.1 < 0
        {
            return Ok(());
        }

        let alpha = color.alpha();
        let rgb = color.rgb();

        if alpha >= 1.0 {
            self.img.put_pixel(
                point.0 as u32,
                point.1 as u32,
                Rgb {
                    data: [rgb.0, rgb.1, rgb.2],
                },
            );
        } else {
            let pixel = self.img.get_pixel_mut(point.0 as u32, point.1 as u32);

            let new_color = [rgb.0, rgb.1, rgb.2];

            pixel
                .data
                .iter_mut()
                .zip(&new_color)
                .for_each(|(old, new)| {
                    *old = (f64::from(*old) * (1.0 - alpha) + f64::from(*new) * alpha).min(255.0)
                        as u8;
                });
        }
        Ok(())
    }
}

impl Drop for BitMapBackend<'_> {
    fn drop(&mut self) {
        if !self.saved {
            self.present().expect("Unable to save the bitmap");
        }
    }
}
