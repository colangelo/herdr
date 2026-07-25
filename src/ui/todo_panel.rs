use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text::{display_width, display_width_u16, truncate_end};
use super::widgets::{
    action_button_row_rects, action_button_width, panel_contrast_fg, render_action_button,
    render_panel_shell, ActionButtonSpec,
};
use crate::app::state::{AppState, PaneTodoPanelButton};
use crate::terminal::todo::{PaneTodo, TodoPriority};

/// Footer buttons in the notification center's language: the shortcut hint
/// inside the filled box, in render order.
const TOGGLE_BUTTON: (&str, &str) = ("spc", "toggle");
const CLEAR_DONE_BUTTON: (&str, &str) = ("c", "clear done");
const CLOSE_BUTTON: (&str, &str) = ("esc", "close");

fn button_spec(button: (&'static str, &'static str)) -> ActionButtonSpec<'static> {
    ActionButtonSpec {
        hint: Some(button.0),
        label: button.1,
    }
}

/// Footer button rects; the mouse layer and the render agree on this geometry.
/// `toggle` is dropped first when the panel is too narrow for all three boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneTodoPanelButtonRects {
    pub toggle: Option<Rect>,
    pub clear_done: Rect,
    pub close: Rect,
}

impl PaneTodoPanelButtonRects {
    pub(crate) fn hit(&self, col: u16, row: u16) -> Option<PaneTodoPanelButton> {
        let contains = |rect: Rect| col >= rect.x && col < rect.x + rect.width && row == rect.y;
        if self.toggle.is_some_and(contains) {
            return Some(PaneTodoPanelButton::Toggle);
        }
        if contains(self.clear_done) {
            return Some(PaneTodoPanelButton::ClearDone);
        }
        if contains(self.close) {
            return Some(PaneTodoPanelButton::Close);
        }
        None
    }

    pub(crate) fn row_y(&self) -> u16 {
        self.clear_done.y
    }
}

pub(crate) fn pane_todo_panel_button_rects(inner: Rect) -> Option<PaneTodoPanelButtonRects> {
    if inner.width == 0 || inner.height < 2 {
        return None;
    }
    let gap = 2u16;
    let all = [
        button_spec(TOGGLE_BUTTON),
        button_spec(CLEAR_DONE_BUTTON),
        button_spec(CLOSE_BUTTON),
    ];
    let all_width: u16 = all
        .iter()
        .map(|spec| action_button_width(spec.hint, spec.label))
        .sum::<u16>()
        + gap * 2;
    let row_offset = inner.height - 1;
    if all_width <= inner.width {
        let rects = action_button_row_rects(inner, &all, gap, row_offset);
        Some(PaneTodoPanelButtonRects {
            toggle: Some(rects[0]),
            clear_done: rects[1],
            close: rects[2],
        })
    } else {
        let rects = action_button_row_rects(inner, &all[1..], gap, row_offset);
        Some(PaneTodoPanelButtonRects {
            toggle: None,
            clear_done: rects[0],
            close: rects[1],
        })
    }
}

/// The `→ label` chip at a row's right edge, for a todo that carries a link.
/// One definition for the renderer and the mouse hit-test, so clicking the
/// chip and seeing the chip cannot drift apart.
pub(crate) fn pane_todo_link_chip(row: Rect, label: &str) -> Option<(Rect, String)> {
    if label.is_empty() || row.width < 16 {
        return None;
    }
    let budget = (row.width / 3) as usize;
    let text = format!(" → {} ", truncate_end(label, budget.saturating_sub(4)));
    let width = display_width_u16(&text);
    if width == 0 || width >= row.width {
        return None;
    }
    Some((Rect::new(row.x + row.width - width, row.y, width, 1), text))
}

/// Three-cell state block, mirroring the notification center's dot column.
fn todo_glyph(todo: &PaneTodo) -> &'static str {
    if todo.done {
        return " ✓ ";
    }
    match todo.priority {
        TodoPriority::High => " ▲ ",
        TodoPriority::Normal => " ● ",
        TodoPriority::Low => " ▼ ",
    }
}

pub(super) fn render_pane_todo_panel(app: &AppState, frame: &mut Frame) {
    let Some(rect) = app.pane_todo_panel_rect() else {
        return;
    };
    let p = &app.palette;
    let Some(inner) = render_panel_shell(frame, rect, p.accent, p.panel_bg) else {
        return;
    };
    let Some(panel) = app.pane_todos.as_ref() else {
        return;
    };
    let todos = app.pane_todos_in_display_order(panel.pane_id);

    if todos.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " no todos",
                Style::default().fg(p.overlay0),
            ))),
            Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
        );
        return;
    }

    let Some((list, start)) = app.pane_todo_panel_list_window() else {
        return;
    };

    for (row, todo) in todos
        .iter()
        .skip(start)
        .take(list.height as usize)
        .enumerate()
    {
        let idx = start + row;
        let row_rect = Rect::new(list.x, list.y + row as u16, list.width, 1);
        let is_selected = idx == panel.selected;
        let chip = todo
            .link
            .as_ref()
            .and_then(|link| pane_todo_link_chip(row_rect, &link.label));
        let chip_width = chip.as_ref().map(|(rect, _)| rect.width).unwrap_or(0) as usize;

        let (glyph_style, text_style, row_style) = if is_selected {
            // The band alone marks selection; the glyph keeps signalling
            // priority and done state so a selected row stays legible.
            let base = Style::default().fg(panel_contrast_fg(p)).bg(p.accent);
            (base, base, base)
        } else if todo.done {
            (
                Style::default().fg(p.overlay0),
                Style::default()
                    .fg(p.overlay0)
                    .add_modifier(Modifier::CROSSED_OUT),
                Style::default(),
            )
        } else {
            (
                Style::default().fg(app.pane_todo_indicator_color(Some(todo.priority))),
                Style::default().fg(p.text),
                Style::default(),
            )
        };

        let text_budget = (list.width as usize).saturating_sub(3 + chip_width);
        let text = truncate_end(&todo.text, text_budget);
        let pad = text_budget.saturating_sub(display_width(&text));
        let line = Line::from(vec![
            Span::styled(todo_glyph(todo), glyph_style),
            Span::styled(text, text_style),
            Span::styled(" ".repeat(pad), row_style),
        ]);
        frame.render_widget(Paragraph::new(line).style(row_style), row_rect);

        if let Some((chip_rect, chip_text)) = chip {
            // A dead link keeps its captured label but reads as inert.
            let chip_style = if is_selected {
                Style::default().fg(panel_contrast_fg(p)).bg(p.accent)
            } else if app.pane_todo_link_target(todo).is_some() {
                Style::default().fg(p.blue)
            } else {
                Style::default().fg(p.overlay0).add_modifier(Modifier::DIM)
            };
            frame.render_widget(Paragraph::new(chip_text).style(chip_style), chip_rect);
        }
    }

    if let Some(buttons) = app.pane_todo_panel_buttons() {
        let hovered = panel.hovered_button;
        let style_for = |button: PaneTodoPanelButton| {
            if hovered == Some(button) {
                Style::default()
                    .fg(panel_contrast_fg(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(p.text)
                    .bg(p.surface0)
                    .add_modifier(Modifier::BOLD)
            }
        };
        if let Some(toggle) = buttons.toggle {
            render_action_button(
                frame,
                toggle,
                Some(TOGGLE_BUTTON.0),
                TOGGLE_BUTTON.1,
                style_for(PaneTodoPanelButton::Toggle),
            );
        }
        render_action_button(
            frame,
            buttons.clear_done,
            Some(CLEAR_DONE_BUTTON.0),
            CLEAR_DONE_BUTTON.1,
            style_for(PaneTodoPanelButton::ClearDone),
        );
        render_action_button(
            frame,
            buttons.close,
            Some(CLOSE_BUTTON.0),
            CLOSE_BUTTON.1,
            style_for(PaneTodoPanelButton::Close),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, layout::Rect, Terminal};

    use crate::app::state::AppState;
    use crate::terminal::todo::{TodoLink, TodoPriority, TodoUpdate};
    use crate::workspace::Workspace;

    /// A workspace with one pane, the panel open on it, and the frame geometry
    /// the notification center tests use.
    fn app_with_open_panel(todos: &[(&str, bool, TodoPriority)]) -> AppState {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        app.view.terminal_area = Rect::new(0, 1, 80, 24);

        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let terminal = app
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist");
        for (text, done, priority) in todos {
            let todo = terminal
                .add_todo(text, *priority, None, 100)
                .expect("todo should be added");
            if *done {
                terminal
                    .update_todo(
                        todo.id,
                        TodoUpdate {
                            done: Some(true),
                            ..TodoUpdate::default()
                        },
                        200,
                    )
                    .expect("todo should be updated");
            }
        }

        app.open_pane_todos(pane_id);
        app
    }

    fn draw(app: &AppState) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        terminal
            .draw(|frame| render_pane_todo_panel(app, frame))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_text(buffer: &ratatui::buffer::Buffer, rect: Rect) -> String {
        (rect.x..rect.x + rect.width)
            .map(|x| buffer[(x, rect.y)].symbol())
            .collect()
    }

    #[test]
    fn rows_render_in_presentation_order() {
        let app = app_with_open_panel(&[
            ("normal one", false, TodoPriority::Normal),
            ("high one", false, TodoPriority::High),
            ("finished", true, TodoPriority::High),
        ]);
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        let buffer = draw(&app);

        assert!(row_text(&buffer, Rect::new(list.x, list.y, list.width, 1)).contains("high one"));
        assert!(
            row_text(&buffer, Rect::new(list.x, list.y + 1, list.width, 1)).contains("normal one")
        );
        assert!(
            row_text(&buffer, Rect::new(list.x, list.y + 2, list.width, 1)).contains("finished"),
            "done todos sink to the bottom"
        );
    }

    #[test]
    fn done_rows_are_dimmed_and_struck() {
        // Two todos on purpose: `open_pane_todos` starts with `selected: 0`,
        // and a selected row is painted by the selection branch (accent band),
        // never the done branch. Done todos sink, so row 1 is the done one and
        // row 0 keeps the cursor.
        let app = app_with_open_panel(&[
            ("still open", false, TodoPriority::Normal),
            ("finished", true, TodoPriority::Normal),
        ]);
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        let buffer = draw(&app);

        // Column 3 is the first text cell after the three-cell state block.
        let cell = &buffer[(list.x + 3, list.y + 1)];
        assert_eq!(cell.style().fg, Some(app.palette.overlay0));
        assert!(cell
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::CROSSED_OUT));
    }

    #[test]
    fn a_dead_link_chip_renders_dimmed_and_a_live_one_does_not() {
        // The live/dead distinction only exists on an unselected row: a
        // selected row's chip takes the accent band like the rest of the row.
        // The decoy is High priority so it sorts to row 0 and holds the
        // starting selection, while `todos()` keeps insertion order, so the
        // linked todo is still `todos()[0]` and lands on row 1.
        let mut app = app_with_open_panel(&[
            ("go look", false, TodoPriority::Normal),
            ("decoy", false, TodoPriority::High),
        ]);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app.terminals[&terminal_id].todos()[0].id;

        // A live link points at a pane that still exists.
        app.terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .update_todo(
                todo_id,
                TodoUpdate {
                    link: Some(Some(TodoLink {
                        pane: Some(pane_id),
                        label: "infra".into(),
                    })),
                    ..TodoUpdate::default()
                },
                300,
            )
            .expect("todo should be updated");
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        // Row 1: the linked todo, which is not the selected row.
        let row = Rect::new(list.x, list.y + 1, list.width, 1);
        let (chip, _) = pane_todo_link_chip(row, "infra").expect("chip should fit");
        let buffer = draw(&app);
        assert!(row_text(&buffer, chip).contains('→'));
        assert_eq!(
            buffer[(chip.x + 1, chip.y)].style().fg,
            Some(app.palette.blue)
        );

        // A dead link keeps its label and reads as inert.
        app.terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .update_todo(
                todo_id,
                TodoUpdate {
                    link: Some(Some(TodoLink {
                        pane: None,
                        label: "infra".into(),
                    })),
                    ..TodoUpdate::default()
                },
                400,
            )
            .expect("todo should be updated");
        let buffer = draw(&app);
        assert!(row_text(&buffer, chip).contains("infra"));
        assert_eq!(
            buffer[(chip.x + 1, chip.y)].style().fg,
            Some(app.palette.overlay0),
            "a dead link is dimmed"
        );
    }

    #[test]
    fn an_empty_pane_shows_the_empty_state_and_no_footer() {
        let app = app_with_open_panel(&[]);
        let buffer = draw(&app);
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");

        assert!(row_text(
            &buffer,
            Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, 1)
        )
        .contains("no todos"));
        assert!(app.pane_todo_panel_buttons().is_none());
    }

    #[test]
    fn the_footer_sits_below_the_list_in_the_settings_button_language() {
        let app = app_with_open_panel(&[("only one", false, TodoPriority::Normal)]);
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        let buttons = app
            .pane_todo_panel_buttons()
            .expect("footer buttons should exist");
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");

        assert_eq!(list.y + list.height, buttons.row_y());
        assert_eq!(buttons.row_y(), rect.y + rect.height - 2);

        // A short todo pins the panel to its 30-cell minimum, and 28 inner
        // cells cannot hold all three boxes (12 + 14 + 11 plus two 2-cell gaps
        // = 41), so `toggle` drops first — the same degradation the
        // notification center applies to its `mark read` box at the same
        // `clamp(30, 60)` minimum.
        assert!(buttons.toggle.is_none());

        let buffer = draw(&app);
        let footer = row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1));
        assert!(footer.contains("c clear done"));
        assert!(footer.contains("esc close"));
    }

    #[test]
    fn a_wide_panel_shows_all_three_footer_buttons() {
        // 40 cells of text push the panel to 46 (40 + borders + glyph block +
        // trailing space), whose 44 inner cells clear the 41 all three boxes
        // need.
        let wide = "x".repeat(40);
        let app = app_with_open_panel(&[(wide.as_str(), false, TodoPriority::Normal)]);
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");
        let buttons = app
            .pane_todo_panel_buttons()
            .expect("footer buttons should exist");

        assert!(buttons.toggle.is_some());

        let buffer = draw(&app);
        let footer = row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1));
        assert!(footer.contains("spc toggle"));
        assert!(footer.contains("c clear done"));
        assert!(footer.contains("esc close"));
    }

    #[test]
    fn the_panel_hangs_from_the_pane_it_belongs_to() {
        let mut app = app_with_open_panel(&[("only one", false, TodoPriority::Normal)]);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        app.view.pane_infos = vec![crate::layout::PaneInfo {
            id: pane_id,
            rect: Rect::new(20, 4, 40, 10),
            inner_rect: Rect::new(21, 5, 38, 8),
            scrollbar_rect: None,
            borders: ratatui::widgets::Borders::ALL,
            is_focused: true,
        }];

        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");
        assert_eq!(rect.x + rect.width, 60, "right-aligned with the pane");
        assert_eq!(rect.y, 5, "hangs off the pane's top border");
    }

    #[test]
    fn selection_clamps_to_the_list_and_survives_an_empty_pane() {
        let mut app = app_with_open_panel(&[
            ("first", false, TodoPriority::Normal),
            ("second", false, TodoPriority::Normal),
        ]);

        app.pane_todos_move_selection(5);
        assert_eq!(
            app.pane_todos.as_ref().expect("panel state").selected,
            1,
            "selection stops at the last row"
        );
        app.pane_todos_move_selection(-9);
        assert_eq!(app.pane_todos.as_ref().expect("panel state").selected, 0);

        let empty = app_with_open_panel(&[]);
        assert!(empty.selected_pane_todo().is_none());
    }
}
