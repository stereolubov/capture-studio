//! Уведомления внутри окна (§11.7): внизу справа, автоисчезновение через 5 с.

use super::widgets as w;
use super::{App, Mode};
use crate::theme;
use egui::{Area, Context, Id, Order, Rect, Sense, pos2, vec2};
use std::time::{Duration, Instant};

const LIFETIME: Duration = Duration::from_secs(5);
const MAX_VISIBLE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Info,
    Warn,
    Error,
}

struct Toast {
    text: String,
    kind: Kind,
    born: Instant,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, text: String, kind: Kind) {
        self.items.push(Toast { text, kind, born: Instant::now() });
        if self.items.len() > MAX_VISIBLE {
            self.items.remove(0);
        }
    }
}

pub fn draw(app: &mut App, ctx: &Context, screen: Rect) {
    app.toasts.items.retain(|t| t.born.elapsed() < LIFETIME);
    if app.toasts.items.is_empty() || app.mode == Mode::Mini {
        return;
    }
    let p = app.palette();
    let font = theme::sans(theme::TEXT);
    let mut bottom = screen.bottom() - 44.0 - 12.0;
    let mut dismiss = None;
    for (i, t) in app.toasts.items.iter().enumerate().rev() {
        let galley = ctx.fonts_mut(|f| f.layout(t.text.clone(), font.clone(), p.fg, 360.0));
        let size = galley.size() + vec2(32.0, 20.0);
        let rect = Rect::from_min_size(pos2(screen.right() - 280.0 - 16.0 - size.x, bottom - size.y), size);
        let accent = match t.kind {
            Kind::Info => p.ok,
            Kind::Warn => p.warn,
            Kind::Error => p.live,
        };
        Area::new(Id::unique(("toast", i))).order(Order::Tooltip).fixed_pos(rect.min).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(size, Sense::click());
            let painter = ui.painter();
            painter.rect_filled(r, 4.0, p.panel);
            w::border(painter, r, 4.0, p.line);
            painter.rect_filled(Rect::from_min_size(r.min + vec2(0.0, 6.0), vec2(2.0, r.height() - 12.0)), 1.0, accent);
            painter.galley(r.min + vec2(16.0, 10.0), galley, p.fg);
            if resp.clicked() {
                dismiss = Some(i);
            }
        });
        bottom = rect.top() - 8.0;
    }
    if let Some(i) = dismiss {
        app.toasts.items.remove(i);
    }
}
