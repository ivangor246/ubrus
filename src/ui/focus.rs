//! Keyboard activation and focus outlines for Iced pointer controls.
use super::Message;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, overlay,
        renderer::{self, Renderer as _},
        widget::{Id, Operation, Tree, operation::focusable::Focusable, tree},
    },
    keyboard, mouse,
};

#[derive(Default)]
struct Focus(bool);
impl Focusable for Focus {
    fn is_focused(&self) -> bool {
        self.0
    }
    fn focus(&mut self) {
        self.0 = true;
    }
    fn unfocus(&mut self) {
        self.0 = false;
    }
}

struct Control<'a> {
    id: Option<Id>,
    child: Element<'a, Message>,
    forward: Option<Message>,
    backward: Option<Message>,
}

pub(super) fn action<'a>(child: impl Into<Element<'a, Message>>, press: Option<Message>) -> Element<'a, Message> {
    cycle(child, press, None)
}

pub(super) fn named_action<'a>(
    id: String,
    child: impl Into<Element<'a, Message>>,
    press: Message,
) -> Element<'a, Message> {
    Element::new(Control {
        id: Some(id.into()),
        child: child.into(),
        forward: Some(press),
        backward: None,
    })
}

pub(super) fn cycle<'a>(
    child: impl Into<Element<'a, Message>>,
    forward: Option<Message>,
    backward: Option<Message>,
) -> Element<'a, Message> {
    Element::new(Control {
        id: None,
        child: child.into(),
        forward,
        backward,
    })
}

impl Widget<Message, Theme, Renderer> for Control<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Focus>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(Focus::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.child)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.child));
    }
    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, _: &Renderer, op: &mut dyn Operation) {
        if self.forward.is_some() {
            op.focusable(self.id.as_ref(), layout.bounds(), tree.state.downcast_mut::<Focus>());
        }
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if let Some(clip) = bounds.intersection(viewport) {
            renderer.with_layer(clip, |r| {
                self.child
                    .as_widget()
                    .draw(&tree.children[0], r, theme, style, layout, cursor, &clip)
            });
            if tree.state.downcast_ref::<Focus>().0 && self.forward.is_some() {
                // The child's clip is a separate layer. Paint the ring on top of it.
                renderer.with_layer(clip, |r| {
                    for (width, color) in [(3., theme.palette().text), (1., theme.palette().background)] {
                        r.fill_quad(
                            renderer::Quad {
                                bounds,
                                border: iced::Border::default().width(width).rounded(6).color(color),
                                ..Default::default()
                            },
                            iced::Color::TRANSPARENT,
                        );
                    }
                });
            }
        }
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Shift+arrow positions the wallpaper even when the pointer is over a slider.
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            && modifiers.shift()
            && matches!(
                key.as_ref(),
                keyboard::Key::Named(
                    keyboard::key::Named::ArrowUp
                        | keyboard::key::Named::ArrowDown
                        | keyboard::key::Named::ArrowLeft
                        | keyboard::key::Named::ArrowRight
                )
            )
        {
            return;
        }
        let focus = tree.state.downcast_mut::<Focus>();
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))) {
            let is_focused = self.forward.is_some() && cursor.is_over(layout.bounds()) && cursor.is_over(*viewport);
            if focus.0 != is_focused {
                focus.0 = is_focused;
                shell.request_redraw();
            }
        }
        if focus.0
            && let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            && modifiers.is_empty()
        {
            use keyboard::key::Named;
            let message = match key.as_ref() {
                keyboard::Key::Named(Named::Enter | Named::Space) => self.forward.as_ref(),
                keyboard::Key::Named(Named::ArrowRight | Named::ArrowDown) if self.backward.is_some() => {
                    self.forward.as_ref()
                }
                keyboard::Key::Named(Named::ArrowLeft | Named::ArrowUp) => self.backward.as_ref(),
                _ => None,
            };
            if let Some(msg) = message {
                shell.publish(msg.clone());
                shell.capture_event();
                return;
            }
        }
        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.child
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.child
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}
