use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text::{display_width, relative_time_label, truncate_end};
use super::widgets::{panel_contrast_fg, render_action_button, render_panel_shell};
use crate::app::state::ToastKind;
use crate::app::AppState;

/// Footer button label. `(c)` echoes the `c` keyboard shortcut.
pub(crate) const CLEAR_BUTTON_LABEL: &str = "Clear all (c)";

/// Rendered width of the footer button (label plus the one-cell pads
/// `render_action_button` adds on each side), so the panel geometry in the
/// mouse layer and the render here agree on the box size.
pub(crate) fn clear_button_width() -> u16 {
    CLEAR_BUTTON_LABEL.chars().count() as u16 + 2
}

/// Hit area for the floating indicator used when
/// `ui.notification_center_position = "bottom-right"`: the last row of the
/// frame, right-aligned, mirroring where the dropdown opens.
pub(super) fn floating_notification_indicator_rect(area: Rect, width: u16) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }
    let width = width.min(area.width);
    Rect::new(
        area.x + area.width - width,
        area.y + area.height - 1,
        width,
        1,
    )
}

/// Draw the bottom-right floating indicator. A no-op for the default
/// top-right position, where the indicator lives in the tab bar instead.
pub(super) fn render_floating_notification_indicator(app: &AppState, frame: &mut Frame) {
    if app.notification_center_position
        != crate::config::NotificationCenterPositionConfig::BottomRight
    {
        return;
    }
    let rect = app.view.notification_hit_area;
    if rect.width == 0 {
        return;
    }
    let p = &app.palette;
    let unread = app.notification_log.unread_count();
    // Floating over pane content, so both states carry a background to read
    // as chrome; the unread pill matches the tab-bar indicator.
    let style = if unread > 0 {
        Style::default()
            .fg(panel_contrast_fg(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.overlay1).bg(p.surface0)
    };
    frame.render_widget(
        Paragraph::new(super::tabs::notification_indicator_label(unread)).style(style),
        rect,
    );
}

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
        // Same button language as the settings/modal action buttons: a filled
        // box, secondary (surface) at rest and accent on hover.
        let style = if hovered {
            Style::default()
                .fg(panel_contrast_fg(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(p.text)
                .bg(p.surface0)
                .add_modifier(Modifier::BOLD)
        };
        render_action_button(frame, button_rect, None, CLEAR_BUTTON_LABEL, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{AppState, ToastKind, ToastNotification};
    use ratatui::layout::Rect;
    use ratatui::{backend::TestBackend, Terminal};

    fn toast(title: &str) -> ToastNotification {
        ToastNotification {
            kind: ToastKind::Finished,
            title: title.to_string(),
            context: String::new(),
            position: None,
            target: None,
        }
    }

    #[test]
    fn floating_indicator_rect_hugs_the_bottom_right_corner() {
        let area = Rect::new(0, 0, 80, 25);
        let rect = floating_notification_indicator_rect(area, 5);
        assert_eq!(rect, Rect::new(75, 24, 5, 1));

        assert_eq!(
            floating_notification_indicator_rect(Rect::default(), 5),
            Rect::default()
        );
    }

    #[test]
    fn floating_indicator_renders_pill_at_bottom_right_when_configured() {
        let mut app = AppState::test_new();
        app.notification_center_position =
            crate::config::NotificationCenterPositionConfig::BottomRight;
        app.post_notification(toast("one"));
        app.post_notification(toast("two"));
        let area = Rect::new(0, 0, 80, 25);
        app.view.notification_hit_area = floating_notification_indicator_rect(
            area,
            super::super::tabs::notification_indicator_width(app.notification_log.unread_count()),
        );

        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_floating_notification_indicator(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let rect = app.view.notification_hit_area;
        assert_eq!(rect.y, 24, "indicator sits on the frame's last row");
        assert_eq!(rect.x + rect.width, 80, "right-aligned to the frame edge");
        let row: String = (rect.x..rect.x + rect.width)
            .map(|x| buffer[(x, rect.y)].symbol())
            .collect();
        assert!(row.contains("◆ 2"), "indicator row: {row:?}");
        let mid = &buffer[(rect.x + 1, rect.y)];
        assert_eq!(mid.style().bg, Some(app.palette.accent), "unread pill bg");
    }

    #[test]
    fn footer_renders_a_filled_clear_button_above_the_bottom_border() {
        let mut app = AppState::test_new();
        app.view.terminal_area = Rect::new(0, 1, 80, 24);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        app.post_notification(toast("one"));
        app.post_notification(toast("two"));
        app.open_notification_center();

        let panel = app.notification_center_rect().expect("panel rect");
        let button = app
            .notification_center_clear_button_rect()
            .expect("clear button rect");

        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_notification_center(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();

        // The button sits on the row directly above the panel's bottom border.
        assert_eq!(button.y, panel.y + panel.height - 2);

        // Its cells carry the settings-style filled background (surface0 at
        // rest), distinguishing it from the plain panel rows.
        let mid = &buffer[(button.x + button.width / 2, button.y)];
        assert_eq!(mid.style().bg, Some(app.palette.surface0));

        let row: String = (button.x..button.x + button.width)
            .map(|x| buffer[(x, button.y)].symbol())
            .collect();
        assert!(row.contains("Clear all (c)"), "button row: {row:?}");
    }
}
