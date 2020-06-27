use iced_native::{
    layout, Background, Color, Element, Hasher, Layout, Length, MouseCursor, Point, Rectangle,
    Widget,
};
use iced_wgpu::{Defaults, Primitive, Renderer};

pub struct Volume {
    volume: f32,
    width: Length,
    height: Length,
}

impl Volume {
    pub fn new(volume: f32) -> Self {
        Self {
            volume,
            width: Length::Fill,
            height: Length::Fill,
        }
    }
    pub fn width(self, width: Length) -> Self {
        Self { width, ..self }
    }
}

impl<Message> Widget<Message, Renderer> for Volume {
    fn width(&self) -> Length {
        self.width
    }
    fn height(&self) -> Length {
        self.height
    }
    fn layout(&self, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.width(self.width).height(self.height).max())
    }
    fn hash_layout(&self, state: &mut Hasher) {
        use std::hash::Hash;
        self.width.hash(state);
        self.height.hash(state);
    }
    fn draw(
        &self,
        _renderer: &mut Renderer,
        _defaults: &Defaults,
        layout: Layout<'_>,
        _cursor_position: Point,
    ) -> (Primitive, MouseCursor) {
        let bounds = layout.bounds();
        let inner_height = self.volume * bounds.height;
        (
            Primitive::Group {
                primitives: vec![
                    Primitive::Quad {
                        bounds: bounds,
                        background: Background::Color(Color::TRANSPARENT),
                        border_radius: 0,
                        border_width: 2,
                        border_color: Color::BLACK,
                    },
                    Primitive::Quad {
                        bounds: Rectangle {
                            height: inner_height - 4.,
                            y: bounds.y + bounds.height - inner_height + 2.,
                            x: bounds.x + 2.,
                            width: bounds.width - 4.,
                        },
                        background: Background::Color(Color::from_rgb(1., 0., 0.)),
                        border_radius: 0,
                        border_width: 0,
                        border_color: Color::TRANSPARENT,
                    },
                ],
            },
            MouseCursor::OutOfBounds,
        )
    }
}

impl<'a, Message> Into<Element<'a, Message, Renderer>> for Volume {
    fn into(self) -> Element<'a, Message, Renderer> {
        Element::new(self)
    }
}
