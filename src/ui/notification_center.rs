use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text::{display_width, relative_time_label, truncate_end};
use super::widgets::{panel_contrast_fg, render_panel_shell};
use crate::app::state::ToastKind;
use crate::app::AppState;

pub(super) fn render_notification_center(app: &AppState, frame: &mut Frame) {
    let Some(rect) = app.notification_center_rect() else {
        return;
    };
    let p = &app.palette;
    let Some(inner) = render_panel_shell(frame, rect, p.accent, p.panel_bg) else {
        return;
    };

    if app.notification_log.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " no notifications",
                Style::default().fg(p.overlay0),
            ))),
            Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
        );
        return;
    }

    let Some((list, start)) = app.notification_center_list_window() else {
        return;
    };
    let selected = app
        .notification_center
        .as_ref()
        .map(|center| center.selected)
        .unwrap_or(0);
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    for (row, entry) in app
        .notification_log
        .entries_newest_first()
        .skip(start)
        .take(list.height as usize)
        .enumerate()
    {
        let idx = start + row;
        let row_rect = Rect::new(list.x, list.y + row as u16, list.width, 1);
        let is_selected = idx == selected;

        let age = relative_time_label(now_unix, entry.posted_at_unix);
        let age_width = display_width(&age);
        let text_budget = (list.width as usize).saturating_sub(3 + age_width + 1);
        let title = truncate_end(&entry.title, text_budget);
        let context_budget = text_budget.saturating_sub(display_width(&title));
        let context = if entry.context.is_empty() || context_budget < 6 {
            String::new()
        } else {
            truncate_end(&format!(" · {}", entry.context), context_budget)
        };
        let pad_width = (list.width as usize)
            .saturating_sub(3 + display_width(&title) + display_width(&context) + age_width + 1);

        let (dot_style, title_style, dim_style, row_style) = if is_selected {
            let fg = panel_contrast_fg(p);
            let selected_base = Style::default().fg(fg).bg(p.accent);
            (
                selected_base,
                selected_base.add_modifier(Modifier::BOLD),
                selected_base,
                selected_base,
            )
        } else {
            let dot_color = match entry.kind {
                ToastKind::NeedsAttention => p.red,
                ToastKind::Finished => p.blue,
                ToastKind::UpdateInstalled => p.accent,
            };
            (
                Style::default().fg(dot_color),
                Style::default().fg(p.text),
                Style::default().fg(p.overlay0),
                Style::default(),
            )
        };

        let line = Line::from(vec![
            Span::styled(" ● ", dot_style),
            Span::styled(title, title_style),
            Span::styled(context, dim_style),
            Span::styled(" ".repeat(pad_width), row_style),
            Span::styled(age, dim_style),
            Span::styled(" ", row_style),
        ]);
        frame.render_widget(Paragraph::new(line).style(row_style), row_rect);
    }

    if let Some(button_rect) = app.notification_center_clear_button_rect() {
        let hovered = app
            .notification_center
            .as_ref()
            .is_some_and(|center| center.clear_hovered);
        let style = if hovered {
            Style::default()
                .fg(panel_contrast_fg(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.overlay1)
        };
        let label = "Clear all (c)";
        let inner_width = button_rect.width as usize;
        let label = truncate_end(label, inner_width);
        let label_width = display_width(&label);
        let left = inner_width.saturating_sub(label_width) / 2;
        let right = inner_width.saturating_sub(label_width + left);
        // Full-width centered text so the hover background fills the row.
        let text = format!("{}{}{}", " ".repeat(left), label, " ".repeat(right));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text, style))),
            button_rect,
        );
    }
}
