use iced_native::{
    layout, Background, Color, Element, Hasher, Layout, Length, MouseCursor, Point, Rectangle, Widget,
};
use iced_wgpu::{Defaults, Primitive, Renderer};

pub struct Volume {
    height: f32,
}

impl Volume {
    pub fn new(height: f32) -> Self {
        Self {height}
    }
}

impl<Message> Widget<Message, Renderer> for Volume {
    fn width(&self) -> Length {
        Length::Fill
    }
    fn height(&self) -> Length {
        Length::Fill
    }
    fn layout(&self, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.max())
    }
    fn hash_layout(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Volume>().hash(state);
    }
    fn draw(&self, _renderer: &mut Renderer, _defaults: &Defaults, layout: Layout<'_>, _cursor_position: Point) -> (Primitive, MouseCursor) {
        let bounds = layout.bounds();
        let inner_height = self.height * bounds.height;
        (
            Primitive::Group {
                primitives: vec![
                Primitive::Quad {
                    bounds: bounds,
                    background: Background::Color(Color::TRANSPARENT),
                    border_radius: 0,
                    border_width: 0,
                    border_color: Color::BLACK,
                },
                Primitive::Quad {
                    bounds: Rectangle {
                        height: inner_height,
                        y: bounds.y + bounds.height - inner_height,
                        ..bounds
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
