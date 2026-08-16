//! The session todo board: every pane's todos in one place.
//!
//! A todo is written *in* a pane, because that is where the work is, but it is
//! read *across* panes, when deciding what to pick up next. The pane panel
//! answers "what about this pane?"; this answers "what is outstanding?", and it
//! is actionable where `herdr todo list --all` is not — activating a row goes
//! to the pane that owns the todo.
//!
//! Deliberately built from the same parts as the panel: the kit's `ButtonRow`
//! footer, its `ListCursor`, and the panel's own row renderer. The board is the
//! panel widened, not a second feature.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::overlay::{ButtonRow, ButtonSpec};
use super::text::truncate_end;
use super::todo_panel::render_pane_todo_row;
use super::widgets::{
    centered_popup_rect, footer_split, panel_contrast_fg, render_action_button,
    render_modal_header, render_modal_shell, FOOTER_ROWS,
};
use crate::app::state::{AppState, TodoBoardButton, TodoBoardItem};

/// Columns the board asks for before its own footer has a say.
const TODO_BOARD_WIDTH: u16 = 64;

/// Rows of chrome around the list: the modal border, the header, and the
/// footer's blank row plus button row.
const TODO_BOARD_CHROME_ROWS: u16 = 3 + FOOTER_ROWS;

/// The board's footer, in the panel's language: the shortcut hint inside the
/// filled box, in render order.
///
/// `toggle` and `clear done` are absent when no pane holds a todo — there is
/// nothing to toggle or clear — and `go` is absent unless the selected todo's
/// link resolves, since there is nowhere to go otherwise. `open` and `close`
/// always survive: `open` is why the board exists over the CLI's flat listing,
/// and an overlay that could not be dismissed is the dead end a footer
/// prevents.
///
/// There is no `add`: adding is pane-scoped, and the board has no pane of its
/// own to add to.
pub(crate) fn todo_board_button_specs(
    has_todos: bool,
    has_live_link: bool,
) -> Vec<ButtonSpec<TodoBoardButton>> {
    let mut specs = Vec::with_capacity(5);
    specs.push(ButtonSpec {
        button: TodoBoardButton::Open,
        hint: Some("↵"),
        label: "open pane",
        drop_rank: None,
    });
    if has_todos {
        specs.push(ButtonSpec {
            button: TodoBoardButton::Toggle,
            hint: Some("spc"),
            label: "toggle",
            drop_rank: Some(0),
        });
    }
    if has_live_link {
        specs.push(ButtonSpec {
            button: TodoBoardButton::Go,
            hint: Some("g"),
            label: "go",
            drop_rank: Some(2),
        });
    }
    if has_todos {
        specs.push(ButtonSpec {
            button: TodoBoardButton::ClearDone,
            hint: Some("c"),
            label: "clear done",
            drop_rank: Some(1),
        });
    }
    specs.push(ButtonSpec {
        button: TodoBoardButton::Close,
        hint: Some("esc"),
        label: "close",
        drop_rank: None,
    });
    specs
}

/// Everything the board is drawn and hit-tested against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TodoBoardGeometry {
    pub outer: Rect,
    pub inner: Rect,
    /// The header row, above the list.
    pub header: Rect,
    /// The list rect, below the header and short of the footer block.
    pub list: Rect,
    pub footer_row: Rect,
}

/// Where the board sits. Centred rather than anchored: it belongs to the
/// session, not to a pane, so there is nothing to hang it off.
///
/// The width is never less than the footer's natural width, so the board is
/// never narrower than the controls it means to show — and it is measured from
/// the full set of boxes rather than the ones the current selection happens to
/// offer, so the box does not resize as the selection moves.
pub(crate) fn todo_board_geometry(area: Rect, item_count: usize) -> Option<TodoBoardGeometry> {
    let width = TODO_BOARD_WIDTH
        .max(ButtonRow::natural_width(&todo_board_button_specs(true, true)).saturating_add(2));
    let height = (item_count as u16)
        .saturating_add(TODO_BOARD_CHROME_ROWS)
        .clamp(8, 20);
    let outer = centered_popup_rect(area, width, height)?;
    let inner = Rect::new(
        outer.x + 1,
        outer.y + 1,
        outer.width.saturating_sub(2),
        outer.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height < 2 + FOOTER_ROWS {
        return None;
    }
    let header = Rect::new(inner.x, inner.y, inner.width, 1);
    let below_header = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let (list, _) = footer_split(below_header, true);
    let footer_row = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    Some(TodoBoardGeometry {
        outer,
        inner,
        header,
        list,
        footer_row,
    })
}

/// A group heading's text: the addressable identifier first, because that is
/// what you can act on with `herdr pane` or a sibling agent's prompt, then the
/// label, because that is what you recognise. The same order the todo link
/// chip settled on.
pub(crate) fn todo_board_heading_text(public_id: Option<&str>, label: &str) -> String {
    match public_id {
        Some(id) if label.is_empty() => format!(" {id}"),
        Some(id) => format!(" {id} · {label}"),
        None => format!(" {label}"),
    }
}

pub(super) fn render_todo_board(app: &AppState, frame: &mut Frame) {
    let Some(board) = app.todo_board() else {
        return;
    };
    let area = frame.area();
    let Some(geometry) = todo_board_geometry(area, board.items.len()) else {
        return;
    };
    let p = &app.palette;
    super::dim_background(frame, area);
    if render_modal_shell(frame, area, geometry.outer.width, geometry.outer.height, p).is_none() {
        return;
    }
    render_modal_header(frame, geometry.header, "todos", p);

    // The empty state still falls through to the footer: opening the board on
    // a quiet session must say so rather than look like a broken keybinding.
    if board.items.is_empty() {
        frame.render_widget(
            Paragraph::new(" nothing outstanding").style(Style::default().fg(p.overlay0)),
            Rect::new(
                geometry.list.x,
                geometry.list.y,
                geometry.list.width,
                geometry.list.height.min(1),
            ),
        );
    }

    let (start, visible) = board.list.window(geometry.list, board.items.len());
    for (row, item) in board.items.iter().skip(start).take(visible).enumerate() {
        let row_rect = Rect::new(
            geometry.list.x,
            geometry.list.y + row as u16,
            geometry.list.width,
            1,
        );
        match item {
            TodoBoardItem::PaneHeading { public_id, label } => {
                // The weight the move picker gives its space headings, so a
                // group reads as a heading and never as a row.
                frame.render_widget(
                    Paragraph::new(truncate_end(
                        &todo_board_heading_text(public_id.as_deref(), label),
                        row_rect.width as usize,
                    ))
                    .style(Style::default().fg(p.overlay0).add_modifier(Modifier::BOLD)),
                    row_rect,
                );
            }
            TodoBoardItem::Todo { pane_id, todo_id } => {
                let Some(todo) = app.pane_todo_ref(*pane_id, *todo_id) else {
                    continue;
                };
                render_pane_todo_row(
                    frame,
                    app,
                    row_rect,
                    todo,
                    start + row == board.list.selected,
                );
            }
        }
    }

    if let Some(buttons) = app.todo_board_buttons() {
        let hovered = board.hovered_button;
        for placed in buttons.placed() {
            let style = if hovered == Some(placed.button) {
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
            render_action_button(frame, placed.rect, placed.hint, placed.label, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    use crate::terminal::todo::{TodoPriority, TodoUpdate};
    use crate::ui::test_support;

    /// A one-pane app whose pane carries `todos`, laid out for the snapshot
    /// frame with the board left closed — the background an open board is
    /// diffed against.
    fn app_with_todos(todos: &[(&str, bool, TodoPriority)]) -> AppState {
        let mut app = test_support::app_with_one_pane("board");
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
        test_support::layout(&mut app);
        app
    }

    fn snapshot(todos: &[(&str, bool, TodoPriority)]) -> test_support::OverlaySnapshot {
        let base = app_with_todos(todos);
        let mut open = app_with_todos(todos);
        open.open_todo_board();
        test_support::layout(&mut open);
        test_support::overlay_snapshot(&base, &open)
    }

    #[test]
    fn snapshot_populated() {
        snapshot(&[
            ("rerun the deploy", false, TodoPriority::High),
            ("check the 403", false, TodoPriority::Normal),
            ("archive the change", true, TodoPriority::Low),
        ])
        .assert(
            Rect::new(7, 8, 66, 9),
            &[
                "┌────────────────────────────────────────────────────────────────┐",
                "│ todos                                                          │",
                "│ w2:p1 · pane 1                                                 │",
                "│ ▲ rerun the deploy                                             │",
                "│ ● check the 403                                                │",
                "│ ✓ archive the change                                           │",
                "│                                                                │",
                "│     ↵ open pane    spc toggle    c clear done    esc close     │",
                "└────────────────────────────────────────────────────────────────┘",
            ],
        );
    }

    #[test]
    fn snapshot_empty() {
        snapshot(&[]).assert(
            Rect::new(7, 8, 66, 8),
            &[
                "┌────────────────────────────────────────────────────────────────┐",
                "│ todos                                                          │",
                "│ nothing outstanding                                            │",
                "│                                                                │",
                "│                                                                │",
                "│                                                                │",
                "│                    ↵ open pane    esc close                    │",
                "└────────────────────────────────────────────────────────────────┘",
            ],
        );
    }

    /// The board is the panel widened, so a todo drawn on it is the same text
    /// the panel draws — same glyph, same first line, same done styling.
    #[test]
    fn a_todo_row_reads_exactly_as_it_does_on_the_pane_panel() {
        let todos = &[("check the 403", false, TodoPriority::Normal)];

        let mut board = app_with_todos(todos);
        board.open_todo_board();
        test_support::layout(&mut board);
        let board_buffer = test_support::draw_sized(
            &board,
            test_support::SNAPSHOT_WIDTH,
            test_support::SNAPSHOT_HEIGHT,
        );
        let (list, _) = board.todo_board_list_window().expect("the board is open");
        // Row 0 is the pane heading; row 1 is the todo.
        let board_row =
            test_support::row_text(&board_buffer, Rect::new(list.x, list.y + 1, list.width, 1));

        let mut panel = app_with_todos(todos);
        let pane_id = panel.workspaces[0].tabs[0].root_pane;
        panel.open_pane_todos(pane_id);
        test_support::layout(&mut panel);
        let panel_buffer = test_support::draw_sized(
            &panel,
            test_support::SNAPSHOT_WIDTH,
            test_support::SNAPSHOT_HEIGHT,
        );
        let (panel_list, _) = panel
            .pane_todo_panel_list_window()
            .expect("the panel is open");
        let panel_row = test_support::row_text(
            &panel_buffer,
            Rect::new(panel_list.x, panel_list.y, panel_list.width, 1),
        );

        assert_eq!(board_row.trim_end(), panel_row.trim_end());
    }

    /// The kit's footer convention: a blank row between the last entry and the
    /// buttons, rather than buttons drawn flush against it.
    #[test]
    fn the_footer_keeps_its_blank_row_above_the_buttons() {
        let geometry = todo_board_geometry(
            Rect::new(
                0,
                0,
                test_support::SNAPSHOT_WIDTH,
                test_support::SNAPSHOT_HEIGHT,
            ),
            4,
        )
        .expect("geometry resolves");
        assert_eq!(
            geometry.list.y + geometry.list.height,
            geometry.footer_row.y - 1
        );
        assert_eq!(geometry.header.y + 1, geometry.list.y);
    }

    /// The board is never narrower than the controls it means to show — the
    /// bug the notification center's footer had.
    #[test]
    fn the_board_is_at_least_as_wide_as_its_own_footer() {
        let geometry = todo_board_geometry(
            Rect::new(
                0,
                0,
                test_support::SNAPSHOT_WIDTH,
                test_support::SNAPSHOT_HEIGHT,
            ),
            4,
        )
        .expect("geometry resolves");
        let natural = ButtonRow::natural_width(&todo_board_button_specs(true, true));
        assert!(
            geometry.inner.width >= natural,
            "inner {} should fit the footer's {natural}",
            geometry.inner.width
        );
    }

    #[test]
    fn a_heading_leads_with_the_addressable_id() {
        assert_eq!(
            todo_board_heading_text(Some("w2:pC"), "claude"),
            " w2:pC · claude"
        );
        assert_eq!(todo_board_heading_text(Some("w2:pC"), ""), " w2:pC");
        assert_eq!(todo_board_heading_text(None, "claude"), " claude");
    }
}
