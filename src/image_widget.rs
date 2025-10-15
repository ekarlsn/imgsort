use iced::{
    mouse,
    widget::canvas::{self, Frame, Geometry},
    Point, Rectangle, Size, Theme,
};

use crate::sorting::Dim;
use crate::{ImageData, Message};

#[derive(Debug, Clone)]
pub enum PixelCanvasMessage {
    CanvasSized(Dim),
}

pub struct PixelCanvas<'a> {
    image_data: Option<&'a ImageData>,
    send_resize_messages: bool,
}

impl<'a> PixelCanvas<'a> {
    pub fn new(image_data: Option<&'a ImageData>, send_resize_messages: bool) -> Self {
        Self {
            image_data,
            send_resize_messages,
        }
    }
}

impl<'a> canvas::Program<Message> for PixelCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let image_data = if let Some(image_data) = &self.image_data {
            image_data
        } else {
            // TODO show loading image here
            return vec![frame.into_geometry()];
        };

        // Calculate scaling to fit the image within bounds while maintaining aspect ratio
        let image_aspect = image_data.width as f32 / image_data.height as f32;
        let bounds_aspect = bounds.width / bounds.height;

        let (draw_width, draw_height) = if image_aspect > bounds_aspect {
            // Image is wider than bounds - scale by width
            (bounds.width, bounds.width / image_aspect)
        } else {
            // Image is taller than bounds - scale by height
            (bounds.height * image_aspect, bounds.height)
        };

        // Center the image in the bounds
        let x_offset = (bounds.width - draw_width) / 2.0;
        let y_offset = (bounds.height - draw_height) / 2.0;

        // Calculate pixel size for rendering
        let pixel_width = draw_width / image_data.width as f32;
        let pixel_height = draw_height / image_data.height as f32;

        // Draw each pixel as a small filled rectangle
        for y in 0..image_data.height {
            for x in 0..image_data.width {
                let pixel_index = ((y * image_data.width + x) * 4) as usize;
                if pixel_index + 3 < image_data.data.len() {
                    let r = image_data.data[pixel_index] as f32 / 255.0;
                    let g = image_data.data[pixel_index + 1] as f32 / 255.0;
                    let b = image_data.data[pixel_index + 2] as f32 / 255.0;
                    let a = image_data.data[pixel_index + 3] as f32 / 255.0;

                    let color = iced::Color::from_rgba(r, g, b, a);

                    frame.fill_rectangle(
                        Point::new(
                            x_offset + x as f32 * pixel_width,
                            y_offset + y as f32 * pixel_height,
                        ),
                        Size::new(pixel_width, pixel_height),
                        color,
                    );
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        _event: canvas::Event,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        // Only send size change messages if enabled
        let message = if self.send_resize_messages {
            Some(Message::PixelCanvas(PixelCanvasMessage::CanvasSized(Dim {
                width: bounds.width as u32,
                height: bounds.height as u32,
            })))
        } else {
            None
        };

        (canvas::event::Status::Ignored, message)
    }
}
