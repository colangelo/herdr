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
const ADD_BUTTON: (&str, &str) = ("a", "add");
const TOGGLE_BUTTON: (&str, &str) = ("spc", "toggle");
const GO_BUTTON: (&str, &str) = ("g", "go");
const CLEAR_DONE_BUTTON: (&str, &str) = ("c", "clear done");
const CLOSE_BUTTON: (&str, &str) = ("esc", "close");

fn button_spec(button: (&'static str, &'static str)) -> ActionButtonSpec<'static> {
    ActionButtonSpec {
        hint: Some(button.0),
        label: button.1,
    }
}

/// Footer button rects; the mouse layer and the render agree on this geometry.
/// `toggle` and `clear_done` are absent on a pane with no todos — there is
/// nothing to toggle or clear — and `go` is absent unless the selected todo's
/// link resolves, since there is nowhere to go otherwise. `add` and `close`
/// always survive: an empty panel that could not add is the dead end this
/// footer exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneTodoPanelButtonRects {
    pub add: Rect,
    pub toggle: Option<Rect>,
    pub go: Option<Rect>,
    pub clear_done: Option<Rect>,
    pub close: Rect,
}

impl PaneTodoPanelButtonRects {
    pub(crate) fn hit(&self, col: u16, row: u16) -> Option<PaneTodoPanelButton> {
        let contains = |rect: Rect| col >= rect.x && col < rect.x + rect.width && row == rect.y;
        if contains(self.add) {
            return Some(PaneTodoPanelButton::Add);
        }
        if self.toggle.is_some_and(contains) {
            return Some(PaneTodoPanelButton::Toggle);
        }
        if self.go.is_some_and(contains) {
            return Some(PaneTodoPanelButton::Go);
        }
        if self.clear_done.is_some_and(contains) {
            return Some(PaneTodoPanelButton::ClearDone);
        }
        if contains(self.close) {
            return Some(PaneTodoPanelButton::Close);
        }
        None
    }

    pub(crate) fn row_y(&self) -> u16 {
        self.close.y
    }
}

pub(crate) fn pane_todo_panel_button_rects(
    inner: Rect,
    has_todos: bool,
    has_live_link: bool,
) -> Option<PaneTodoPanelButtonRects> {
    // Needs room for the whole footer block, blank row included, or the
    // buttons would sit flush against the last todo.
    if inner.width == 0 || inner.height < super::widgets::FOOTER_ROWS {
        return None;
    }
    let gap = 2u16;
    let row_offset = inner.height - 1;

    // Widest layout first, each step dropping the least essential box still
    // standing. `go` is the last of the three to go: following a link is the
    // one action whose key nothing else on screen advertises. The final pair is
    // returned whether or not it fits, matching how the row behaved before
    // `add` joined it.
    for (want_toggle, want_clear, want_go) in [
        (true, true, true),
        (false, true, true),
        (false, false, true),
        (false, false, false),
    ] {
        let with_toggle = want_toggle && has_todos;
        let with_clear = want_clear && has_todos;
        let with_go = want_go && has_live_link;

        let mut specs = Vec::with_capacity(5);
        specs.push(button_spec(ADD_BUTTON));
        if with_toggle {
            specs.push(button_spec(TOGGLE_BUTTON));
        }
        if with_go {
            specs.push(button_spec(GO_BUTTON));
        }
        if with_clear {
            specs.push(button_spec(CLEAR_DONE_BUTTON));
        }
        specs.push(button_spec(CLOSE_BUTTON));

        let width: u16 = specs
            .iter()
            .map(|spec| action_button_width(spec.hint, spec.label))
            .sum::<u16>()
            + gap * (specs.len() as u16 - 1);
        let last = !want_toggle && !want_clear && !want_go;
        if width > inner.width && !last {
            continue;
        }

        let mut rects = action_button_row_rects(inner, &specs, gap, row_offset).into_iter();
        let add = rects.next()?;
        let toggle = with_toggle.then(|| rects.next()).flatten();
        let go = with_go.then(|| rects.next()).flatten();
        let clear_done = with_clear.then(|| rects.next()).flatten();
        let close = rects.next()?;
        return Some(PaneTodoPanelButtonRects {
            add,
            toggle,
            go,
            clear_done,
            close,
        });
    }
    None
}

/// The link chip at a row's right edge, for a todo that carries a link. One
/// definition for the renderer and the mouse hit-test, so clicking the chip and
/// seeing the chip cannot drift apart.
///
/// A live link leads with the target's public identifier and follows it with
/// the captured label — `→ w2:pC · claude` — because the identifier is the part
/// you can act on and the label is the part you recognise. A dead link has no
/// identifier to lead with, so it keeps its label alone.
pub(crate) fn pane_todo_link_chip(
    row: Rect,
    public_id: Option<&str>,
    label: &str,
) -> Option<(Rect, String)> {
    if (label.is_empty() && public_id.is_none()) || row.width < 16 {
        return None;
    }
    // The chip takes what it needs so long as it leaves the todo's own text a
    // readable minimum, past the three-cell state glyph. The panel sizes
    // itself from `pane_todo_link_chip_text`, so on an untruncated chip this
    // budget is not the binding constraint.
    let budget = (row.width as usize).saturating_sub(3 + CHIP_MIN_TEXT_COLUMNS);
    // Four cells of frame: the leading space, the arrow and its space, and the
    // trailing space.
    let content = budget.saturating_sub(4);
    // The identifier is short and takes its width off the top; the label is
    // what gives when the row is narrow.
    let label = match public_id {
        Some(id) => truncate_end(label, content.saturating_sub(display_width(id) + 3)),
        None => truncate_end(label, content),
    };
    let text = pane_todo_link_chip_text(public_id, &label);
    let width = display_width_u16(&text);
    if width == 0 || width >= row.width {
        return None;
    }
    Some((Rect::new(row.x + row.width - width, row.y, width, 1), text))
}

/// Columns the todo's own text keeps whatever its link is called.
const CHIP_MIN_TEXT_COLUMNS: usize = 8;

/// The chip's text before any truncation. One definition, so the panel sizes
/// itself for exactly what the chip is going to draw.
pub(crate) fn pane_todo_link_chip_text(public_id: Option<&str>, label: &str) -> String {
    match public_id {
        Some(id) if label.is_empty() => format!(" → {id} "),
        Some(id) => format!(" → {id} · {label} "),
        None => format!(" → {label} "),
    }
}

/// What a todo shows on its single panel row. A todo may hold more than one
/// line; the panel lists one row each, so the rest is signalled rather than
/// shown.
pub(crate) fn pane_todo_row_text(text: &str, budget: usize) -> String {
    let Some((first, _)) = text.split_once('\n') else {
        return truncate_end(text, budget);
    };
    let marker = " ⏎";
    let first = truncate_end(first, budget.saturating_sub(display_width(marker)));
    format!("{first}{marker}")
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

    // The empty state still falls through to the footer below: an empty pane
    // is the one you most want to add a todo to, so its panel must not be a
    // dead end.
    if todos.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " no todos",
                Style::default().fg(p.overlay0),
            ))),
            Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
        );
    }

    if let Some((list, start)) = app.pane_todo_panel_list_window() {
        for (row, todo) in todos
            .iter()
            .skip(start)
            .take(list.height as usize)
            .enumerate()
        {
            let idx = start + row;
            let row_rect = Rect::new(list.x, list.y + row as u16, list.width, 1);
            let is_selected = idx == panel.selected;
            let public_id = app.pane_todo_link_public_id(todo);
            let chip = todo
                .link
                .as_ref()
                .and_then(|link| pane_todo_link_chip(row_rect, public_id.as_deref(), &link.label));
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
            let text = pane_todo_row_text(&todo.text, text_budget);
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
        render_action_button(
            frame,
            buttons.add,
            Some(ADD_BUTTON.0),
            ADD_BUTTON.1,
            style_for(PaneTodoPanelButton::Add),
        );
        if let Some(toggle) = buttons.toggle {
            render_action_button(
                frame,
                toggle,
                Some(TOGGLE_BUTTON.0),
                TOGGLE_BUTTON.1,
                style_for(PaneTodoPanelButton::Toggle),
            );
        }
        if let Some(go) = buttons.go {
            render_action_button(
                frame,
                go,
                Some(GO_BUTTON.0),
                GO_BUTTON.1,
                style_for(PaneTodoPanelButton::Go),
            );
        }
        if let Some(clear_done) = buttons.clear_done {
            render_action_button(
                frame,
                clear_done,
                Some(CLEAR_DONE_BUTTON.0),
                CLEAR_DONE_BUTTON.1,
                style_for(PaneTodoPanelButton::ClearDone),
            );
        }
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
        let public_id = app.pane_todo_link_public_id(&app.terminals[&terminal_id].todos()[0]);
        let (chip, _) =
            pane_todo_link_chip(row, public_id.as_deref(), "infra").expect("chip should fit");
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
        // Recomputed rather than reused: a dead chip drops the identifier, so
        // it is shorter and the right-aligned rect moves with it.
        let (chip, _) = pane_todo_link_chip(row, None, "infra").expect("chip should fit");
        let buffer = draw(&app);
        assert!(row_text(&buffer, chip).contains("infra"));
        assert_eq!(
            buffer[(chip.x + 1, chip.y)].style().fg,
            Some(app.palette.overlay0),
            "a dead link is dimmed"
        );
    }

    /// Spec: "the link is presented with that pane's public identifier first
    /// and its captured label after it", and "a moved target is addressed by
    /// where it is now". The identifier is the part you can act on — it is
    /// what `herdr pane` and a sibling agent's prompt take — so it leads.
    #[test]
    fn a_live_link_chip_leads_with_the_public_id_and_a_dead_one_shows_its_label_alone() {
        let mut app = app_with_open_panel(&[
            ("go look", false, TodoPriority::Normal),
            ("decoy", false, TodoPriority::High),
        ]);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app.terminals[&terminal_id].todos()[0].id;
        let link = |app: &mut AppState, target: Option<crate::layout::PaneId>, at: u64| {
            app.terminals
                .get_mut(&terminal_id)
                .expect("test terminal should exist")
                .update_todo(
                    todo_id,
                    TodoUpdate {
                        link: Some(Some(TodoLink {
                            pane: target,
                            label: "infra".into(),
                        })),
                        ..TodoUpdate::default()
                    },
                    at,
                )
                .expect("todo should be updated");
        };

        link(&mut app, Some(pane_id), 300);
        let public_id = app
            .session_public_pane_id(pane_id)
            .expect("a live pane has a public id");
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        // Row 1: the linked todo, which is not the selected row.
        let row = Rect::new(list.x, list.y + 1, list.width, 1);
        let (chip, text) =
            pane_todo_link_chip(row, Some(&public_id), "infra").expect("chip should fit");
        assert_eq!(text, format!(" → {public_id} · infra "));

        // The drawn cells are the cells the hit-test is handed, so what looks
        // clickable is clickable.
        let buffer = draw(&app);
        assert_eq!(row_text(&buffer, chip), text);

        link(&mut app, None, 400);
        let (dead_chip, dead_text) =
            pane_todo_link_chip(row, None, "infra").expect("chip should fit");
        assert_eq!(dead_text, " → infra ", "no identifier to lead with");
        let buffer = draw(&app);
        assert_eq!(row_text(&buffer, dead_chip), dead_text);
    }

    /// Spec: "it occupies a single row showing the first line with a marker".
    /// The panel sizes itself from `todos.len()`, so a todo that grew a second
    /// line must not grow a second row under it.
    #[test]
    fn a_multi_line_todo_occupies_one_row_showing_its_first_line() {
        let app = app_with_open_panel(&[
            ("first line\nsecond line", false, TodoPriority::High),
            ("plain", false, TodoPriority::Normal),
        ]);
        let (list, _) = app
            .pane_todo_panel_list_window()
            .expect("panel list window should exist");
        let buffer = draw(&app);

        let row = row_text(&buffer, Rect::new(list.x, list.y, list.width, 1));
        assert!(row.contains("first line"));
        assert!(row.contains('⏎'), "the marker says more follows: {row}");
        assert!(!row.contains("second line"));
        assert!(
            row_text(&buffer, Rect::new(list.x, list.y + 1, list.width, 1)).contains("plain"),
            "the next todo still starts on the very next row"
        );
    }

    #[test]
    fn the_row_text_marker_fits_inside_the_budget() {
        assert_eq!(pane_todo_row_text("one line", 20), "one line");
        assert_eq!(pane_todo_row_text("one\ntwo", 20), "one ⏎");
        // The marker is reserved out of the budget rather than overrunning it.
        let squeezed = pane_todo_row_text("a long first line\nmore", 10);
        assert!(display_width(&squeezed) <= 10, "{squeezed}");
        assert!(squeezed.ends_with('⏎'));
    }

    /// Spec: "an empty panel can still add". The empty state used to render no
    /// footer at all, which — now that every pane carries an indicator to open
    /// it with — would make clicking a quiet pane a dead end.
    #[test]
    fn an_empty_pane_shows_the_empty_state_and_can_still_add() {
        let app = app_with_open_panel(&[]);
        let buffer = draw(&app);
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");

        assert!(row_text(
            &buffer,
            Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, 1)
        )
        .contains("no todos"));

        let buttons = app
            .pane_todo_panel_buttons()
            .expect("an empty panel still offers a footer");
        assert!(
            buttons.toggle.is_none() && buttons.clear_done.is_none(),
            "nothing to toggle or clear on a pane with no todos"
        );

        let footer = row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1));
        assert!(footer.contains("a add"), "the way out of the empty state");
        assert!(footer.contains("esc close"));
    }

    /// Following a link worked from the first release of the panel — `g`, and
    /// a click on the chip — but the footer never said so, which made a chip
    /// that reads like a label the only visible route.
    #[test]
    fn the_footer_offers_go_only_while_the_selected_todo_has_a_live_link() {
        // Wide text so the panel is roomy enough to hold every box.
        let wide = "x".repeat(60);
        let mut app = app_with_open_panel(&[(wide.as_str(), false, TodoPriority::Normal)]);
        assert!(
            app.pane_todo_panel_buttons()
                .expect("footer buttons")
                .go
                .is_none(),
            "an unlinked todo has nowhere to go"
        );

        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app.terminals[&terminal_id].todos()[0].id;
        let link = |app: &mut AppState, target: Option<crate::layout::PaneId>, at: u64| {
            app.terminals
                .get_mut(&terminal_id)
                .expect("test terminal should exist")
                .update_todo(
                    todo_id,
                    TodoUpdate {
                        link: Some(Some(TodoLink {
                            pane: target,
                            label: "infra".into(),
                        })),
                        ..TodoUpdate::default()
                    },
                    at,
                )
                .expect("todo should be updated");
        };

        link(&mut app, Some(pane_id), 300);
        let buttons = app.pane_todo_panel_buttons().expect("footer buttons");
        let go = buttons.go.expect("a live link offers the go button");
        assert_eq!(
            buttons.hit(go.x, go.y),
            Some(PaneTodoPanelButton::Go),
            "the drawn box is the box the mouse hits"
        );
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");
        let buffer = draw(&app);
        assert!(
            row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1)).contains("g go"),
            "the footer advertises the key that was already there"
        );

        // A dead link is inert, so the button goes away rather than lying.
        link(&mut app, None, 400);
        assert!(app
            .pane_todo_panel_buttons()
            .expect("footer buttons")
            .go
            .is_none());
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

        // One blank row separates the last todo from the buttons — the panel
        // convention, so nothing sits flush against the footer.
        assert_eq!(
            list.y + list.height + 1,
            buttons.row_y(),
            "one blank row between the list and the buttons"
        );
        assert_eq!(buttons.row_y(), rect.y + rect.height - 2);

        // A short todo pins the panel to its 30-cell minimum, and 28 inner
        // cells hold only two boxes: all four need 7 + 12 + 14 + 11 plus three
        // 2-cell gaps = 50, dropping `toggle` still needs 36. So `add` and
        // `close` are what is left. `add` outranks `clear done` here because
        // it is this panel's primary action and the one a mouse user has no
        // other route to — clearing done work keeps its `c` key.
        assert!(buttons.toggle.is_none());
        assert!(buttons.clear_done.is_none());

        let buffer = draw(&app);
        let footer = row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1));
        assert!(footer.contains("a add"));
        assert!(footer.contains("esc close"));

        // The separator row really is blank — geometry alone would not catch a
        // stray draw into it.
        let separator = row_text(
            &buffer,
            Rect::new(list.x, list.y + list.height, list.width, 1),
        );
        assert!(
            separator.trim().is_empty(),
            "the row above the buttons should be blank, got {separator:?}"
        );
    }

    #[test]
    fn a_wide_panel_shows_all_four_footer_buttons() {
        // 50 cells of text push the panel to 56 (50 + borders + glyph block +
        // trailing space), whose 54 inner cells clear the 50 all four boxes
        // need.
        let wide = "x".repeat(50);
        let app = app_with_open_panel(&[(wide.as_str(), false, TodoPriority::Normal)]);
        let rect = app.pane_todo_panel_rect().expect("panel rect should exist");
        let buttons = app
            .pane_todo_panel_buttons()
            .expect("footer buttons should exist");

        assert!(buttons.toggle.is_some());
        assert!(buttons.clear_done.is_some());

        let buffer = draw(&app);
        let footer = row_text(&buffer, Rect::new(rect.x, buttons.row_y(), rect.width, 1));
        assert!(footer.contains("a add"));
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
