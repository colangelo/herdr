use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    Frame,
};

mod dialogs;
mod keybind_help;
pub(crate) mod list_motion;
mod menus;
mod mobile;
mod navigator;
mod notification_center;
mod onboarding;
mod panes;
mod release_notes;
mod scrollbar;
mod settings;
mod sidebar;
mod status;
mod tab_surface;
mod tabs;
mod todo_panel;
// pub(crate): the CLI reuses text helpers (e.g. relative_time_label).
pub(crate) mod text;
// pub(crate): pure-data editing state that modal key handling drives.
pub(crate) mod text_field;
mod widgets;

use self::dialogs::{
    render_confirm_close_overlay, render_new_linked_worktree_overlay,
    render_open_existing_worktree_overlay, render_pane_move_target_picker_overlay,
    render_pane_todo_edit_overlay, render_remove_worktree_overlay, render_rename_overlay,
};
use self::keybind_help::render_keybind_help_overlay;
use self::menus::{
    render_context_menu, render_copy_mode_overlay, render_global_launcher_menu,
    render_navigate_overlay, render_prefix_overlay, render_resize_overlay,
};
use self::mobile::{
    compute_mobile_header_hit_areas, is_mobile_width, mobile_switcher_max_scroll_for_height,
    mobile_toast_banner_rect, render_mobile_header, render_mobile_panel,
    render_mobile_toast_banner,
};
use self::navigator::render_navigator_overlay;
use self::notification_center::{
    floating_notification_indicator_rect, render_floating_notification_indicator,
    render_notification_center,
};
pub(crate) use self::notification_center::{
    notification_center_button_rects, NotificationCenterButtonRects,
};
pub(crate) use self::onboarding::onboarding_welcome_continue_rect;
use self::onboarding::render_onboarding_overlay;
pub(crate) use self::panes::popup_pane_rects;
use self::panes::{render_empty, render_popup_pane, resize_popup_pane};
pub(crate) use self::release_notes::{
    product_announcement_display_lines, release_notes_close_button_rect,
    release_notes_display_lines, release_notes_wrapped_line_count, PRODUCT_ANNOUNCEMENT_MODAL_SIZE,
    RELEASE_NOTES_MODAL_SIZE,
};
use self::release_notes::{render_product_announcement_overlay, render_release_notes_overlay};
pub(crate) use self::scrollbar::{
    pane_scrollbar_rect, release_notes_scrollbar_rect, scrollbar_offset_from_drag_row,
    scrollbar_offset_from_row, scrollbar_thumb_grab_offset, should_show_scrollbar,
};
use self::settings::render_settings_overlay;
#[cfg(test)]
pub(crate) use self::sidebar::workspace_drop_indicator_row;
use self::sidebar::{render_sidebar, render_sidebar_collapsed};
use self::status::{
    copy_feedback_rect, render_config_diagnostic, render_copy_feedback, render_toast_notification,
    toast_notification_rect,
};
pub(crate) use self::tab_surface::{
    compute_tab_surface, render_tab_surface, resize_tab_surface, TabSurfaceLayout,
};
use self::tabs::render_tab_bar;
use self::todo_panel::render_pane_todo_panel;
pub(crate) use self::todo_panel::{pane_todo_panel_button_rects, PaneTodoPanelButtonRects};
// The chip's cells have exactly one definition, so a click can never land on
// cells the renderer did not draw: the renderer reaches it inside the module
// and the mouse hit-test reaches it through this re-export.
pub(crate) use self::todo_panel::{pane_todo_link_chip, pane_todo_link_chip_text};
pub(crate) use self::{
    dialogs::{
        confirm_close_button_rects, confirm_close_popup_rect, new_linked_worktree_button_rects,
        new_linked_worktree_inner_rect, open_existing_worktree_button_rects,
        open_existing_worktree_inner_rect, open_existing_worktree_max_visible_rows,
        open_existing_worktree_visible_start, pane_move_target_button_rects,
        pane_move_target_inner_rect, pane_todo_edit_column_scroll, pane_todo_edit_line_scroll,
        pane_todo_edit_rects, pane_todo_edit_text_area, remove_worktree_button_rects,
        remove_worktree_popup_rect, rename_button_rects, PaneTodoEditRects,
        PANE_TODO_EDIT_POPUP_HEIGHT, PANE_TODO_EDIT_POPUP_WIDTH,
    },
    settings::{
        settings_button_rects, settings_popup_height, settings_show_primary_action,
        SETTINGS_POPUP_WIDTH,
    },
    sidebar::{
        agent_entry_gap, agent_entry_height_in_body, agent_panel_body_rect, agent_panel_entries,
        agent_panel_motion_active, agent_panel_scroll_for_target, agent_panel_scroll_metrics,
        agent_panel_scrollbar_rect, agent_panel_target_keys, agent_panel_toggle_rect,
        all_agent_panel_entries, collapsed_sidebar_sections, collapsed_sidebar_toggle_rect,
        compute_workspace_card_areas, expanded_sidebar_sections, expanded_sidebar_toggle_rect,
        normalized_workspace_scroll, sidebar_section_divider_rect, workspace_drop_slots,
        workspace_group_chevron_rect, workspace_list_entries, workspace_list_entries_expanded,
        workspace_list_rect, workspace_list_scroll_metrics, workspace_list_scrollbar_rect,
        workspace_motion_active, workspace_parent_group_state, workspace_unit_target_keys,
        AgentPanelEntry, WorkspaceListEntry,
    },
};

pub(crate) use self::{
    keybind_help::keybind_help_lines,
    mobile::{
        mobile_switcher_areas, mobile_switcher_max_scroll, mobile_switcher_target_at,
        mobile_switcher_workspace_doc_range, MobileSwitcherTarget,
    },
    panes::{apply_pane_chrome, pane_inner_rect, pane_is_scrolled_back},
    tab_surface::{tab_surface_cursor, tab_surface_hyperlinks, TabSurfaceView},
    tabs::{compute_tab_bar_view, notification_indicator_width},
    tabs::{compute_tab_bar_view, tab_bar_content_area},
    widgets::{centered_popup_rect, modal_stack_areas},
};
// The indicator's cells have exactly one definition; the pane renderer reaches
// it through the module with the terminal it already resolved, and the mouse
// hit-test reaches it through this re-export so a click can never land on cells
// the renderer did not draw. The returned `PaneTodoIndicator` is read field by
// field, so no caller outside this module needs to name the type.
pub(crate) use self::panes::pane_todo_indicator;
use crate::app::state::ViewLayout;
use crate::app::{AppState, Mode};
use crate::terminal::TerminalRuntimeRegistry;

const COLLAPSED_WIDTH: u16 = 4; // num + space + dot + separator

/// Collapsed sidebar width: `left`/`right` active-border modes reserve one
/// extra edge column for the accent bar, mirroring the expanded lists.
pub(crate) fn collapsed_sidebar_width(app: &AppState) -> u16 {
    COLLAPSED_WIDTH
        + u16::from(matches!(
            app.sidebar_active_border,
            crate::config::SidebarActiveBorderConfig::Left
                | crate::config::SidebarActiveBorderConfig::Right
        ))
}

/// Compute view geometry and reconcile pane sizes.
/// Called before render to separate mutation from drawing.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compute_view(app: &mut AppState, area: Rect) {
    let terminal_runtimes = TerminalRuntimeRegistry::new();
    compute_view_with_runtime_registry(app, &terminal_runtimes, area);
}

pub fn compute_view_with_runtime_registry(
    app: &mut AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    area: Rect,
) {
    compute_view_internal(
        app,
        terminal_runtimes,
        area,
        true,
        crate::kitty_graphics::HostCellSize::default(),
    );
}

pub fn compute_view_with_cell_size(
    app: &mut AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    area: Rect,
    cell_size: crate::kitty_graphics::HostCellSize,
) {
    compute_view_internal(app, terminal_runtimes, area, true, cell_size);
}

/// Compute view geometry for a client-sized render without resizing pane runtimes.
///
/// This is used by the headless server when a non-foreground client needs its
/// own frame size while the shared pane runtimes stay pinned to the foreground
/// client.
pub(crate) fn compute_view_without_resizing_panes(
    app: &mut AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    area: Rect,
) {
    compute_view_internal(
        app,
        terminal_runtimes,
        area,
        false,
        crate::kitty_graphics::HostCellSize::default(),
    );
}

fn resize_background_tab_panes_to_area(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    terminal_area: Rect,
    cell_size: crate::kitty_graphics::HostCellSize,
) {
    for (ws_idx, ws) in app.workspaces.iter().enumerate() {
        for (tab_idx, tab) in ws.tabs.iter().enumerate() {
            if app.active == Some(ws_idx) && tab_idx == ws.active_tab_index() {
                continue;
            }
            resize_tab_surface(app, terminal_runtimes, tab, terminal_area, cell_size);
        }
    }
}

fn resize_background_tab_panes_for_desktop(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    main_area: Rect,
    cell_size: crate::kitty_graphics::HostCellSize,
) {
    for (ws_idx, ws) in app.workspaces.iter().enumerate() {
        let (_, terminal_area) = desktop_tab_bar_and_terminal_area(app, ws, main_area);
        for (tab_idx, tab) in ws.tabs.iter().enumerate() {
            if app.active == Some(ws_idx) && tab_idx == ws.active_tab_index() {
                continue;
            }
            resize_tab_surface(app, terminal_runtimes, tab, terminal_area, cell_size);
        }
    }
}

fn desktop_tab_bar_and_terminal_area(
    app: &AppState,
    ws: &crate::workspace::Workspace,
    main_area: Rect,
) -> (Rect, Rect) {
    let hide_single_tab_bar = app.hide_tab_bar_when_single_tab && ws.tabs.len() == 1;
    if !hide_single_tab_bar && main_area.height > 1 {
        match app.tab_bar_position {
            crate::config::TabBarPositionConfig::Top => {
                let [tab_bar_rect, terminal_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(main_area);
                (tab_bar_rect, terminal_area)
            }
            crate::config::TabBarPositionConfig::Bottom => {
                let [terminal_area, tab_bar_rect] =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(main_area);
                (tab_bar_rect, terminal_area)
            }
        }
    } else {
        (Rect::default(), main_area)
    }
}

/// Keep the sidebar lists following focus: while a list's follow is engaged,
/// every computed view scrolls just enough to keep the active workspace /
/// focused agent entry visible (nearest edge, no recentering) — even when the
/// entry moved because the list reordered (priority re-sorts, entries added
/// or removed). Manual scrolling disengages a list's follow; the next focus
/// change re-engages it, mirroring `tab_scroll_follow_active`. Runs in
/// compute_view so it sees settled state regardless of which path changed
/// focus or order (keybinding, picker, mouse, runtime API, agent state).
fn follow_sidebar_focus(app: &mut AppState, sidebar_area: Rect) {
    let active_ws_id = app
        .active
        .and_then(|idx| app.workspaces.get(idx))
        .map(|ws| ws.id.clone());
    if app.sidebar_followed_workspace != active_ws_id {
        app.sidebar_followed_workspace = active_ws_id;
        app.workspace_list_follow_active = true;
    }
    if app.workspace_list_follow_active {
        if let Some(ws_idx) = app.active {
            app.ensure_workspace_visible_in_rect(sidebar_area, ws_idx);
        }
    }

    let focused_agent = app.active.and_then(|ws_idx| {
        let ws = app.workspaces.get(ws_idx)?;
        let pane_id = ws.focused_pane_id()?;
        Some((ws.id.clone(), ws_idx, pane_id))
    });
    let focused_identity = focused_agent
        .as_ref()
        .map(|(ws_id, _, pane_id)| (ws_id.clone(), *pane_id));
    if app.sidebar_followed_agent != focused_identity {
        app.sidebar_followed_agent = focused_identity;
        app.agent_panel_follow_active = true;
    }
    if app.agent_panel_follow_active {
        if let Some((_, ws_idx, pane_id)) = focused_agent {
            let entry_idx = agent_panel_entries(app)
                .iter()
                .position(|entry| entry.ws_idx == ws_idx && entry.pane_id == pane_id);
            if let Some(idx) = entry_idx {
                app.ensure_agent_panel_entry_visible_in_rect(sidebar_area, idx);
            }
        }
    }
}

fn compute_view_internal(
    app: &mut AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    area: Rect,
    resize_panes: bool,
    cell_size: crate::kitty_graphics::HostCellSize,
) {
    if is_mobile_width(area, app.mobile_width_threshold) {
        compute_mobile_view(app, terminal_runtimes, area, resize_panes, cell_size);
        return;
    }

    let sidebar_w = if app.sidebar_collapsed {
        match app.sidebar_collapsed_mode {
            crate::config::SidebarCollapsedModeConfig::Compact => collapsed_sidebar_width(app),
            crate::config::SidebarCollapsedModeConfig::Hidden => 0,
        }
    } else {
        app.sidebar_width
            .clamp(app.sidebar_min_width, app.sidebar_max_width)
    };

    let [sidebar_area, main_area] =
        Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(1)]).areas(area);

    let (tab_bar_rect, terminal_area) = app
        .active
        .and_then(|i| app.workspaces.get(i))
        .map(|ws| desktop_tab_bar_and_terminal_area(app, ws, main_area))
        .unwrap_or((Rect::default(), main_area));

    if !app.sidebar_collapsed {
        app.workspace_scroll = normalized_workspace_scroll(app, sidebar_area, app.workspace_scroll);
        let (_, detail_area) = expanded_sidebar_sections(sidebar_area, app.sidebar_section_split);
        let max_agent_scroll = agent_panel_scroll_metrics(app, detail_area).max_offset_from_bottom;
        app.agent_panel_scroll = app.agent_panel_scroll.min(max_agent_scroll);
        follow_sidebar_focus(app, sidebar_area);
    } else {
        app.workspace_scroll = app
            .workspace_scroll
            .min(app.workspaces.len().saturating_sub(1));
        app.agent_panel_scroll = 0;
    }

    let workspace_card_areas = if app.sidebar_collapsed {
        Vec::new()
    } else {
        compute_workspace_card_areas(app, sidebar_area)
    };

    let indicator_width = notification_indicator_width(app.notification_log.unread_count());
    let indicator_in_tab_bar = app.notification_center_position
        == crate::config::NotificationCenterPositionConfig::TopRight;
    let tab_bar_view = app
        .active
        .and_then(|ws_idx| app.workspaces.get(ws_idx))
        .map(|ws| {
            compute_tab_bar_view(
                ws,
                tab_bar_content_area(app, tab_bar_rect),
                app.tab_scroll,
                app.tab_scroll_follow_active,
                app.mouse_capture,
                if indicator_in_tab_bar {
                    indicator_width
                } else {
                    0
                },
            )
        })
        .unwrap_or_default();
    app.tab_scroll = tab_bar_view.scroll;
    let notification_hit_area = if indicator_in_tab_bar {
        tab_bar_view.notification_hit_area
    } else {
        floating_notification_indicator_rect(area, indicator_width)
    };

    let TabSurfaceLayout {
        pane_infos,
        split_borders,
    } = compute_tab_surface(
        app,
        terminal_runtimes,
        terminal_area,
        resize_panes,
        cell_size,
    );
    if resize_panes {
        resize_background_tab_panes_for_desktop(app, terminal_runtimes, main_area, cell_size);
        resize_popup_pane(app, terminal_runtimes, terminal_area, cell_size);
    }

    let toast_hit_area = app
        .toast
        .as_ref()
        .map(|toast| {
            toast_notification_rect(
                area,
                terminal_area,
                toast,
                app.config_diagnostic.is_some(),
                toast.position.unwrap_or(app.toast_config.herdr.position),
                app.toast_config.herdr.size,
            )
        })
        .unwrap_or_default();

    app.view = crate::app::ViewState {
        layout: ViewLayout::Desktop,
        sidebar_rect: sidebar_area,
        workspace_card_areas,
        tab_bar_rect,
        tab_hit_areas: tab_bar_view.tab_hit_areas,
        tab_scroll_left_hit_area: tab_bar_view.scroll_left_hit_area,
        tab_scroll_right_hit_area: tab_bar_view.scroll_right_hit_area,
        new_tab_hit_area: tab_bar_view.new_tab_hit_area,
        notification_hit_area,
        terminal_area,
        mobile_header_rect: Rect::default(),
        mobile_menu_hit_area: Rect::default(),
        toast_hit_area,
        pane_infos,
        split_borders,
    };
    app.sync_copy_mode_search_geometry();
}

fn compute_mobile_view(
    app: &mut AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    area: Rect,
    resize_panes: bool,
    cell_size: crate::kitty_graphics::HostCellSize,
) {
    let header_h = area.height.min(2);
    let (header_rect, terminal_area) = if area.height > header_h {
        let [header_rect, terminal_area] =
            Layout::vertical([Constraint::Length(header_h), Constraint::Min(1)]).areas(area);
        (header_rect, terminal_area)
    } else {
        (area, Rect::default())
    };

    if app.mode == Mode::Navigate {
        let switcher_viewport_h = area.height.saturating_sub(header_h + 1);
        let max_scroll = mobile_switcher_max_scroll_for_height(app, switcher_viewport_h);
        app.mobile_switcher_scroll = app.mobile_switcher_scroll.min(max_scroll);
    }

    let TabSurfaceLayout {
        pane_infos,
        split_borders,
    } = compute_tab_surface(
        app,
        terminal_runtimes,
        terminal_area,
        resize_panes,
        cell_size,
    );
    if resize_panes {
        resize_background_tab_panes_to_area(app, terminal_runtimes, terminal_area, cell_size);
        resize_popup_pane(app, terminal_runtimes, terminal_area, cell_size);
    }
    let header_hits = compute_mobile_header_hit_areas(app, header_rect);

    let toast_hit_area = app
        .toast
        .as_ref()
        .map(|_| mobile_toast_banner_rect(area, app.config_diagnostic.is_some()))
        .unwrap_or_default();

    app.view = crate::app::ViewState {
        layout: ViewLayout::Mobile,
        sidebar_rect: Rect::default(),
        workspace_card_areas: Vec::new(),
        tab_bar_rect: Rect::default(),
        tab_hit_areas: Vec::new(),
        tab_scroll_left_hit_area: Rect::default(),
        tab_scroll_right_hit_area: Rect::default(),
        new_tab_hit_area: Rect::default(),
        notification_hit_area: Rect::default(),
        terminal_area,
        mobile_header_rect: header_rect,
        mobile_menu_hit_area: header_hits.menu,
        toast_hit_area,
        pane_infos,
        split_borders,
    };
    app.sync_copy_mode_search_geometry();
}

/// Render the UI — reads AppState but does not mutate it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn render(app: &AppState, frame: &mut Frame) {
    let terminal_runtimes = TerminalRuntimeRegistry::new();
    render_with_runtime_registry(app, &terminal_runtimes, frame);
}

pub fn render_with_runtime_registry(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    frame: &mut Frame,
) {
    let tab_bar_area = app.view.tab_bar_rect;
    let terminal_area = app.view.terminal_area;

    render_navigation_chrome(app, terminal_runtimes, frame);
    if app.view.layout != ViewLayout::Mobile {
        render_tab_bar(app, frame, tab_bar_area);
    }
    if app
        .active
        .and_then(|ws_idx| app.workspaces.get(ws_idx))
        .is_some()
    {
        render_tab_surface(app, terminal_runtimes, app.view.tab_surface(), frame);
    } else {
        render_empty(app, frame, terminal_area);
    }

    if app.view.layout != ViewLayout::Mobile {
        // Bottom-right notification indicator floats over pane content;
        // transient toasts and interactive overlays still draw above it.
        render_floating_notification_indicator(app, frame);
    }

    // Ambient notifications sit above panes, but below interactive overlays.
    render_notifications(app, frame, terminal_area);
    render_popup_pane(app, terminal_runtimes, frame, terminal_area);

    let mode_bar_area = if app.view.layout == ViewLayout::Desktop
        && app.tab_bar_position == crate::config::TabBarPositionConfig::Bottom
        && tab_bar_area.height > 0
    {
        tab_bar_area
    } else {
        terminal_area
    };

    match app.mode {
        Mode::Onboarding => render_onboarding_overlay(app, frame, frame.area()),
        Mode::ReleaseNotes => render_release_notes_overlay(app, frame, frame.area()),
        Mode::ProductAnnouncement => render_product_announcement_overlay(app, frame, frame.area()),
        Mode::Navigate if app.view.layout == ViewLayout::Mobile => {
            render_mobile_panel(app, terminal_runtimes, frame, frame.area())
        }
        Mode::Navigate => render_navigate_overlay(app, frame, mode_bar_area),
        Mode::Prefix => render_prefix_overlay(app, frame, mode_bar_area),
        Mode::Copy => render_copy_mode_overlay(app, frame, mode_bar_area),
        Mode::Resize => render_resize_overlay(app, frame, mode_bar_area),
        Mode::ConfirmClose => {
            render_confirm_close_overlay(app, terminal_runtimes, frame, terminal_area)
        }
        Mode::ContextMenu => {
            render_context_menu(app, frame);
        }
        Mode::Settings => render_settings_overlay(app, frame, frame.area()),
        Mode::RenameWorkspace | Mode::RenameTab | Mode::RenamePane => {
            render_rename_overlay(app, frame, frame.area())
        }
        Mode::NewLinkedWorktree => render_new_linked_worktree_overlay(app, frame, frame.area()),
        Mode::OpenExistingWorktree => {
            render_open_existing_worktree_overlay(app, frame, frame.area())
        }
        Mode::PaneMoveTargetPicker => {
            render_pane_move_target_picker_overlay(app, frame, frame.area())
        }
        Mode::ConfirmRemoveWorktree => render_remove_worktree_overlay(app, frame, frame.area()),
        Mode::GlobalMenu => render_global_launcher_menu(app, frame),
        Mode::KeybindHelp => render_keybind_help_overlay(app, frame),
        Mode::Navigator => render_navigator_overlay(app, terminal_runtimes, frame),
        Mode::NotificationCenter => render_notification_center(app, frame),
        Mode::PaneTodos => render_pane_todo_panel(app, frame),
        Mode::PaneTodoEdit => render_pane_todo_edit_overlay(app, frame, frame.area()),
        Mode::Terminal => {}
    }
}

fn render_navigation_chrome(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    frame: &mut Frame,
) {
    if app.view.layout == ViewLayout::Mobile {
        render_mobile_header(app, terminal_runtimes, frame, app.view.mobile_header_rect);
    } else if app.view.sidebar_rect.width > 0 {
        if app.sidebar_collapsed {
            render_sidebar_collapsed(app, frame, app.view.sidebar_rect);
        } else {
            render_sidebar(app, terminal_runtimes, frame, app.view.sidebar_rect);
        }
    }
}

fn render_notifications(app: &AppState, frame: &mut Frame, terminal_area: Rect) {
    let has_config_diagnostic = app.config_diagnostic.is_some();
    if let Some(message) = &app.config_diagnostic {
        let diagnostic_area = if app.view.layout == ViewLayout::Mobile {
            terminal_area
        } else {
            frame.area()
        };
        render_config_diagnostic(frame, diagnostic_area, message, &app.palette);
    }
    let mut copy_feedback_offset = u16::from(has_config_diagnostic);
    let mut toast_rect = None;
    if let Some(toast) = &app.toast {
        if app.view.layout == ViewLayout::Mobile {
            render_mobile_toast_banner(
                frame,
                frame.area(),
                toast,
                has_config_diagnostic,
                &app.palette,
            );
        } else {
            render_toast_notification(
                frame,
                frame.area(),
                app.view.terminal_area,
                toast,
                has_config_diagnostic,
                toast.position.unwrap_or(app.toast_config.herdr.position),
                app.toast_config.herdr.size,
                &app.palette,
            );
            toast_rect = Some(toast_notification_rect(
                frame.area(),
                app.view.terminal_area,
                toast,
                has_config_diagnostic,
                toast.position.unwrap_or(app.toast_config.herdr.position),
                app.toast_config.herdr.size,
            ));
        }
        if app.view.layout == ViewLayout::Mobile {
            toast_rect = Some(mobile_toast_banner_rect(
                frame.area(),
                has_config_diagnostic,
            ));
        }
    }
    if let Some(feedback) = &app.copy_feedback {
        let area = if app.view.layout == ViewLayout::Mobile {
            frame.area()
        } else {
            terminal_area
        };
        if let Some(toast_rect) = toast_rect {
            copy_feedback_offset = copy_feedback_offset_for_toast(
                area,
                feedback,
                copy_feedback_offset,
                app.toast_config.clipboard.position,
                toast_rect,
            );
        }
        render_copy_feedback(
            frame,
            area,
            feedback,
            copy_feedback_offset,
            app.toast_config.clipboard.position,
            &app.palette,
        );
    }
}

fn copy_feedback_offset_for_toast(
    area: Rect,
    feedback: &crate::app::state::CopyFeedback,
    base_offset: u16,
    position: crate::config::ToastClipboardPosition,
    toast_rect: Rect,
) -> u16 {
    let feedback_rect = copy_feedback_rect(area, feedback, base_offset, position);
    if rects_overlap(feedback_rect, toast_rect) {
        base_offset.saturating_add(toast_rect.height)
    } else {
        base_offset
    }
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x < b.x.saturating_add(b.width)
        && b.x < a.x.saturating_add(a.width)
        && a.y < b.y.saturating_add(b.height)
        && b.y < a.y.saturating_add(a.height)
}

fn dim_background(frame: &mut Frame, area: Rect) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &mut buf[(x, y)];
            cell.set_style(cell.style().add_modifier(Modifier::DIM));
        }
    }
}

/// Floating overlay for navigate mode — appears at bottom of terminal area.
fn _build_hints(items: &[(&str, &str)], key_style: Style, dim_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));
    for (i, (k, desc)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", dim_style));
        }
        spans.push(Span::styled(k.to_string(), key_style));
        spans.push(Span::styled(format!(" {desc}"), dim_style));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::keybind_help::keybind_help_groups;
    use super::scrollbar::scrollbar_thumb;
    use super::*;
    use crate::{app::state::ViewLayout, layout::PaneInfo, workspace::Workspace};
    use ratatui::style::Color;
    use ratatui::{backend::TestBackend, Terminal};

    fn sidebar_focus_test_app(count: usize) -> crate::app::state::AppState {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = (0..count)
            .map(|i| Workspace::test_new(&format!("ws-{i}")))
            .collect();
        app.ensure_test_terminals();
        for ws in &app.workspaces {
            let pane_id = ws.tabs[0].root_pane;
            let terminal_id = ws.tabs[0]
                .panes
                .get(&pane_id)
                .expect("root pane")
                .attached_terminal_id
                .clone();
            if let Some(terminal) = app.terminals.get_mut(&terminal_id) {
                terminal.set_detected_state(
                    Some(crate::detect::Agent::Pi),
                    crate::detect::AgentState::Idle,
                );
            }
        }
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app
    }

    #[test]
    fn compute_view_reveals_newly_active_workspace_in_sidebar() {
        let mut app = sidebar_focus_test_app(30);
        let area = Rect::new(0, 0, 80, 24);
        compute_view(&mut app, area);
        assert!(app
            .view
            .workspace_card_areas
            .iter()
            .any(|card| card.ws_idx == 0));
        assert!(!app
            .view
            .workspace_card_areas
            .iter()
            .any(|card| card.ws_idx == 29));

        app.active = Some(29);
        app.selected = 29;
        compute_view(&mut app, area);
        assert!(app
            .view
            .workspace_card_areas
            .iter()
            .any(|card| card.ws_idx == 29));
    }

    #[test]
    fn compute_view_leaves_sidebar_scroll_alone_after_manual_scroll() {
        let mut app = sidebar_focus_test_app(30);
        let area = Rect::new(0, 0, 80, 24);
        app.active = Some(29);
        app.selected = 29;
        compute_view(&mut app, area);
        assert!(app.workspace_scroll > 0);
        assert!(app.agent_panel_scroll > 0);

        // Manual scrolling disengages the follow (as the scroll input
        // handlers do); the lists then stay where the user put them.
        app.workspace_list_follow_active = false;
        app.agent_panel_follow_active = false;
        app.workspace_scroll = 0;
        app.agent_panel_scroll = 0;
        compute_view(&mut app, area);
        assert_eq!(app.workspace_scroll, 0);
        assert_eq!(app.agent_panel_scroll, 0);

        // The next focus change re-engages the follow.
        app.active = Some(28);
        app.selected = 28;
        compute_view(&mut app, area);
        assert!(app.workspace_list_follow_active);
        assert!(app.agent_panel_follow_active);
        assert!(app.workspace_scroll > 0);
        assert!(app.agent_panel_scroll > 0);
    }

    fn set_agent_state(
        app: &mut crate::app::state::AppState,
        ws_idx: usize,
        state: crate::detect::AgentState,
    ) {
        let pane_id = app.workspaces[ws_idx].tabs[0].root_pane;
        let terminal_id = app.workspaces[ws_idx].tabs[0]
            .panes
            .get(&pane_id)
            .expect("root pane")
            .attached_terminal_id
            .clone();
        app.terminals.get_mut(&terminal_id).expect("terminal").state = state;
    }

    fn focused_agent_entry_visible(app: &crate::app::state::AppState) -> bool {
        let (_, detail_area) =
            expanded_sidebar_sections(app.view.sidebar_rect, app.sidebar_section_split);
        let metrics = agent_panel_scroll_metrics(app, detail_area);
        let ws_idx = app.active.expect("active workspace");
        let pane_id = app.workspaces[ws_idx].focused_pane_id().expect("focus");
        let idx = agent_panel_entries(app)
            .iter()
            .position(|entry| entry.ws_idx == ws_idx && entry.pane_id == pane_id)
            .expect("focused agent entry");
        idx >= app.agent_panel_scroll && idx < app.agent_panel_scroll + metrics.viewport_rows
    }

    #[test]
    fn compute_view_keeps_focused_agent_visible_when_priority_resort_moves_it() {
        let mut app = sidebar_focus_test_app(30);
        app.agent_panel_sort = crate::app::state::AgentPanelSort::Priority;
        let area = Rect::new(0, 0, 80, 24);
        app.active = Some(29);
        app.selected = 29;
        compute_view(&mut app, area);
        assert!(app.agent_panel_scroll > 0);
        assert!(focused_agent_entry_visible(&app));

        // The focused agent starts working: priority sort bubbles its entry
        // to the top without any focus change. The follow keeps it visible.
        set_agent_state(&mut app, 29, crate::detect::AgentState::Working);
        assert_eq!(agent_panel_entries(&app)[0].ws_idx, 29);
        compute_view(&mut app, area);
        assert!(focused_agent_entry_visible(&app));
        assert_eq!(app.agent_panel_scroll, 0);
    }

    #[test]
    fn compute_view_keeps_active_workspace_visible_when_priority_resort_moves_it() {
        let mut app = sidebar_focus_test_app(30);
        app.workspace_sort = crate::app::state::WorkspaceSort::Priority;
        let area = Rect::new(0, 0, 80, 24);
        app.active = Some(29);
        app.selected = 29;
        compute_view(&mut app, area);
        assert!(app.workspace_scroll > 0);

        // The active workspace's agent starts working: priority sort moves
        // its row to the top without a focus change. The follow tracks it.
        set_agent_state(&mut app, 29, crate::detect::AgentState::Working);
        compute_view(&mut app, area);
        assert!(app
            .view
            .workspace_card_areas
            .iter()
            .any(|card| card.ws_idx == 29));
    }

    #[test]
    fn compute_view_reveals_newly_focused_agent_in_agent_panel() {
        let mut app = sidebar_focus_test_app(30);
        let area = Rect::new(0, 0, 80, 24);
        compute_view(&mut app, area);
        assert_eq!(app.agent_panel_scroll, 0);

        app.active = Some(29);
        app.selected = 29;
        compute_view(&mut app, area);

        let (_, detail_area) =
            expanded_sidebar_sections(app.view.sidebar_rect, app.sidebar_section_split);
        let metrics = agent_panel_scroll_metrics(&app, detail_area);
        let idx = agent_panel_entries(&app)
            .iter()
            .position(|entry| entry.ws_idx == 29)
            .expect("agent entry for focused workspace");
        assert!(idx >= app.agent_panel_scroll);
        assert!(idx < app.agent_panel_scroll + metrics.viewport_rows);
    }

    #[test]
    fn copy_feedback_offset_only_increases_when_toast_rect_overlaps() {
        let area = Rect::new(0, 0, 80, 24);
        let feedback = crate::app::state::CopyFeedback {
            message: "copied to clipboard".into(),
        };
        let toast = crate::app::state::ToastNotification {
            kind: crate::app::state::ToastKind::Finished,
            title: "pi finished".into(),
            context: "workspace · 1".into(),
            position: None,
            target: None,
        };

        let bottom_right_toast = toast_notification_rect(
            area,
            Rect::default(),
            &toast,
            false,
            crate::config::ToastHerdrPosition::BottomRight,
            crate::config::ToastHerdrSize::Auto,
        );
        assert_eq!(
            copy_feedback_offset_for_toast(
                area,
                &feedback,
                0,
                crate::config::ToastClipboardPosition::TopCenter,
                bottom_right_toast,
            ),
            0
        );

        let bottom_center_toast = Rect::new(28, 21, 24, 3);
        assert_eq!(
            copy_feedback_offset_for_toast(
                area,
                &feedback,
                0,
                crate::config::ToastClipboardPosition::BottomCenter,
                bottom_center_toast,
            ),
            bottom_center_toast.height
        );
    }

    #[test]
    fn workspace_creation_dialog_renders_new_workspace_title() {
        let mut app = crate::app::state::AppState::test_new();
        app.mode = Mode::RenameWorkspace;
        app.pending_workspace_create_cwd = Some("/tmp/project".into());
        app.name_input = "project".into();

        let area = Rect::new(0, 0, 80, 20);
        compute_view(&mut app, area);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let screen = (0..area.height)
            .map(|row| buffer_row_text(terminal.backend().buffer(), area, row))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(screen.contains("new workspace"), "{screen}");
        assert!(screen.contains("project"), "{screen}");
    }

    #[tokio::test]
    async fn focused_pane_cursor_wins_during_terminal_render() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        let first_pane = ws.tabs[0].root_pane;
        let second_pane = ws.test_split(ratatui::layout::Direction::Horizontal);

        ws.insert_test_runtime(
            first_pane,
            crate::terminal::TerminalRuntime::test_with_screen_bytes(20, 5, b"left"),
        );
        ws.insert_test_runtime(
            second_pane,
            crate::terminal::TerminalRuntime::test_with_screen_bytes(20, 5, b"r\r\nb"),
        );
        ws.tabs[0].layout.focus_pane(first_pane);

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        let focused = app
            .view
            .pane_infos
            .iter()
            .find(|info| info.id == first_pane)
            .expect("focused pane info");

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();

        terminal
            .backend_mut()
            .assert_cursor_position((focused.inner_rect.x + 4, focused.inner_rect.y));
    }

    #[test]
    fn mobile_width_uses_header_and_full_width_terminal() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 44, 20));

        assert_eq!(app.view.layout, ViewLayout::Mobile);
        assert_eq!(app.view.sidebar_rect, Rect::default());
        assert_eq!(app.view.tab_bar_rect, Rect::default());
        assert_eq!(app.view.mobile_header_rect, Rect::new(0, 0, 44, 2));
        assert_eq!(app.view.terminal_area, Rect::new(0, 2, 44, 18));
        assert_eq!(app.view.mobile_menu_hit_area.height, 2);
        assert_eq!(
            app.view.mobile_menu_hit_area.x + app.view.mobile_menu_hit_area.width,
            44
        );
    }

    #[test]
    fn mobile_config_diagnostic_keeps_command_visible() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.config_diagnostic = Some("config.toml:100:10; herdr config check".into());

        let area = Rect::new(0, 0, 44, 20);
        compute_view(&mut app, area);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let row = buffer_row_text(terminal.backend().buffer(), area, app.view.terminal_area.y);

        assert!(row.contains("config.toml:100:10"), "{row}");
        assert!(row.contains("herdr config check"), "{row}");
    }

    #[test]
    fn desktop_toast_hit_area_uses_full_frame_not_terminal_area() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.toast_config.herdr.position = crate::config::ToastHerdrPosition::TopLeft;
        app.toast = Some(crate::app::state::ToastNotification {
            kind: crate::app::state::ToastKind::Finished,
            title: "pi finished".into(),
            context: "one".into(),
            position: None,
            target: None,
        });

        compute_view(&mut app, Rect::new(0, 0, 100, 20));

        assert_eq!(app.view.layout, ViewLayout::Desktop);
        assert!(app.view.terminal_area.x > 0);
        assert_eq!(app.view.toast_hit_area.x, 0);
        assert_eq!(app.view.toast_hit_area.y, 0);
    }

    #[test]
    fn desktop_toast_hit_area_still_offsets_for_config_diagnostic() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.config_diagnostic = Some("config warning".into());
        app.toast_config.herdr.position = crate::config::ToastHerdrPosition::TopLeft;
        app.toast = Some(crate::app::state::ToastNotification {
            kind: crate::app::state::ToastKind::Finished,
            title: "pi finished".into(),
            context: "one".into(),
            position: None,
            target: None,
        });

        compute_view(&mut app, Rect::new(0, 0, 100, 20));

        assert_eq!(app.view.toast_hit_area.x, 0);
        assert_eq!(app.view.toast_hit_area.y, 1);
    }

    #[test]
    fn configured_mobile_width_threshold_controls_layout_switch() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        assert_eq!(app.view.layout, ViewLayout::Desktop);

        app.mobile_width_threshold = 90;
        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        assert_eq!(app.view.layout, ViewLayout::Mobile);
        assert_eq!(app.view.mobile_header_rect, Rect::new(0, 0, 80, 2));
        assert_eq!(app.view.terminal_area, Rect::new(0, 2, 80, 18));
    }

    #[test]
    fn desktop_tab_bar_position_controls_geometry_and_mode_bar_placement() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Prefix;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        assert_eq!(app.view.tab_bar_rect, Rect::new(26, 0, 54, 1));
        assert_eq!(app.view.terminal_area, Rect::new(26, 1, 54, 19));

        app.tab_bar_position = crate::config::TabBarPositionConfig::Bottom;
        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        assert_eq!(app.view.terminal_area, Rect::new(26, 0, 54, 19));
        assert_eq!(app.view.tab_bar_rect, Rect::new(26, 19, 54, 1));
        assert!(app.view.tab_hit_areas.iter().all(|rect| rect.y == 19));
        assert_eq!(app.view.new_tab_hit_area.y, 19);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let mode_row = buffer_row_text(
            terminal.backend().buffer(),
            app.view.tab_bar_rect,
            app.view.tab_bar_rect.y,
        );
        assert!(mode_row.contains("PREFIX"), "{mode_row}");
    }

    #[test]
    fn hide_tab_bar_when_single_tab_toggles_geometry_with_tab_count() {
        let mut app = crate::app::state::AppState::test_new();
        app.hide_tab_bar_when_single_tab = true;
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        let single_tab_terminal_area = app.view.terminal_area;
        assert_eq!(app.view.tab_bar_rect, Rect::default());
        assert_eq!(single_tab_terminal_area, Rect::new(26, 0, 54, 20));
        assert!(app.view.tab_hit_areas.is_empty());
        assert_eq!(app.view.new_tab_hit_area, Rect::default());

        app.workspaces[0].test_add_tab(Some("logs"));
        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        assert_eq!(app.view.tab_bar_rect, Rect::new(26, 0, 54, 1));
        assert_eq!(app.view.terminal_area, Rect::new(26, 1, 54, 19));
        assert_eq!(app.view.tab_hit_areas.len(), 2);
        assert!(app.view.tab_hit_areas.iter().all(|rect| rect.width > 0));
        assert!(app.view.new_tab_hit_area.width > 0);

        assert!(app.workspaces[0].close_tab(1));
        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        assert_eq!(app.view.terminal_area, single_tab_terminal_area);
        assert_eq!(app.view.tab_bar_rect, Rect::default());
        assert!(app.view.tab_hit_areas.is_empty());
        assert_eq!(app.view.new_tab_hit_area, Rect::default());
    }

    #[test]
    fn bottom_tab_bar_still_hides_when_single_tab() {
        let mut app = crate::app::state::AppState::test_new();
        app.hide_tab_bar_when_single_tab = true;
        app.tab_bar_position = crate::config::TabBarPositionConfig::Bottom;
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Prefix;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));
        assert_eq!(app.view.tab_bar_rect, Rect::default());
        assert_eq!(app.view.terminal_area, Rect::new(26, 0, 54, 20));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let mode_row = buffer_row_text(
            terminal.backend().buffer(),
            app.view.terminal_area,
            app.view.terminal_area.y + app.view.terminal_area.height - 1,
        );
        assert!(mode_row.contains("PREFIX"), "{mode_row}");
    }

    #[tokio::test]
    async fn hide_tab_bar_when_single_tab_resizes_background_tabs_per_workspace() {
        let mut app = crate::app::state::AppState::test_new();
        app.hide_tab_bar_when_single_tab = true;

        let mut one_tab_workspace = Workspace::test_new("one");
        let one_tab_pane = one_tab_workspace.tabs[0].root_pane;
        let one_tab_runtime = crate::terminal::TerminalRuntime::test_with_screen_bytes(10, 5, b"");
        one_tab_workspace.tabs[0]
            .runtimes
            .insert(one_tab_pane, one_tab_runtime);

        let mut two_tab_workspace = Workspace::test_new("two");
        let background_tab = two_tab_workspace.test_add_tab(Some("logs"));
        let two_tab_pane = two_tab_workspace.tabs[background_tab].root_pane;
        let two_tab_runtime = crate::terminal::TerminalRuntime::test_with_screen_bytes(10, 5, b"");
        two_tab_workspace.tabs[background_tab]
            .runtimes
            .insert(two_tab_pane, two_tab_runtime);

        app.workspaces = vec![one_tab_workspace, two_tab_workspace];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let one_tab_size = app.workspaces[0].tabs[0].runtimes[&one_tab_pane].current_size();
        let two_tab_size =
            app.workspaces[1].tabs[background_tab].runtimes[&two_tab_pane].current_size();
        assert_eq!(one_tab_size, (20, 53));
        assert_eq!(two_tab_size, (19, 53));
    }

    #[tokio::test]
    async fn mobile_background_tabs_use_mobile_terminal_area() {
        let mut app = crate::app::state::AppState::test_new();

        let mut workspace = Workspace::test_new("mobile");
        let background_tab = workspace.test_add_tab(Some("logs"));
        let background_pane = workspace.tabs[background_tab].root_pane;
        let runtime = crate::terminal::TerminalRuntime::test_with_screen_bytes(10, 5, b"");
        workspace.tabs[background_tab]
            .runtimes
            .insert(background_pane, runtime);

        app.workspaces = vec![workspace];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 44, 20));

        assert_eq!(app.view.layout, ViewLayout::Mobile);
        assert_eq!(app.view.terminal_area, Rect::new(0, 2, 44, 18));
        assert_eq!(
            app.workspaces[0].tabs[background_tab].runtimes[&background_pane].current_size(),
            (18, 43)
        );
    }

    #[test]
    fn product_announcement_renders_above_config_diagnostic() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::ProductAnnouncement;
        app.product_announcement = Some(crate::app::state::ProductAnnouncementState {
            version: "0.6.0".into(),
            id: "keybinding-v2".into(),
            title: "Keybinding syntax changed".into(),
            body: "### Update\n- Body".into(),
            scroll: 0,
            preview: false,
        });
        app.config_diagnostic = Some(
            "unsafe direct keybinding: keys.new_workspace = \"n\"\nunsafe direct keybinding: keys.new_tab = \"c\""
                .into(),
        );

        let area = Rect::new(0, 0, 44, 20);
        compute_view(&mut app, area);

        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let popup = centered_popup_rect(
            area,
            PRODUCT_ANNOUNCEMENT_MODAL_SIZE.0,
            PRODUCT_ANNOUNCEMENT_MODAL_SIZE.1,
        )
        .expect("announcement popup");
        let title_row = popup.y + 1;
        let row = buffer_row_text(buffer, Rect::new(0, title_row, area.width, 1), title_row);

        assert!(row.contains("Keybinding syntax changed"));
        assert!(!row.contains("config warning"));
    }

    #[test]
    fn compute_view_clamps_sidebar_width_to_configured_max() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.sidebar_max_width = 30;
        app.sidebar_width = 999;

        compute_view(&mut app, Rect::new(0, 0, 100, 20));

        assert_eq!(app.view.sidebar_rect.width, 30);
    }

    #[test]
    fn compute_view_clamps_sidebar_width_to_configured_min() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.sidebar_min_width = 22;
        app.sidebar_width = 5;

        compute_view(&mut app, Rect::new(0, 0, 100, 20));

        assert_eq!(app.view.sidebar_rect.width, 22);
    }

    #[test]
    fn hidden_collapsed_sidebar_uses_full_width_terminal_area() {
        let mut app = crate::app::state::AppState::test_new();
        app.sidebar_collapsed = true;
        app.sidebar_collapsed_mode = crate::config::SidebarCollapsedModeConfig::Hidden;
        app.workspaces = vec![Workspace::test_new("one")];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        assert_eq!(app.view.sidebar_rect, Rect::new(0, 0, 0, 20));
        assert_eq!(app.view.tab_bar_rect, Rect::new(0, 0, 80, 1));
        assert_eq!(app.view.terminal_area, Rect::new(0, 1, 80, 19));
        assert!(app.view.workspace_card_areas.is_empty());

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }

    #[test]
    fn collapsed_sidebar_keeps_active_workspace_highlight_in_terminal_mode() {
        let mut app = crate::app::state::AppState::test_new();
        app.sidebar_collapsed = true;
        app.workspaces = vec![Workspace::test_new("one"), Workspace::test_new("two")];
        app.active = Some(1);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let (ws_area, _, _) = collapsed_sidebar_sections(app.view.sidebar_rect);
        let active_row = ws_area.y + 1;
        let active_style = buffer[(ws_area.x, active_row)].style();

        assert_eq!(active_style.bg, Some(app.palette.surface_dim));
    }

    #[test]
    fn expanded_sidebar_workspace_rows_show_state_before_name_without_numbers() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("one");
        let repo = temp_git_repo("main");
        ws.identity_cwd = repo.clone();
        let root_pane = ws.tabs[0].root_pane;
        ws.refresh_git_ahead_behind();

        app.workspaces = vec![ws];
        app.ensure_test_terminals();
        let root_terminal_id = app.workspaces[0].tabs[0].panes[&root_pane]
            .attached_terminal_id
            .clone();
        app.terminals.get_mut(&root_terminal_id).unwrap().cwd = repo.clone();
        app.selected = 0;
        app.mode = Mode::Navigate;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let card = app.view.workspace_card_areas[0].rect;
        let line1 = buffer_row_text(buffer, card, card.y);
        let line2 = buffer_row_text(buffer, card, card.y + 1);

        assert!(line1.starts_with(" · one"));
        assert!(!line1.contains("1 one"));
        assert_eq!(line2, "   main");

        std::fs::remove_dir_all(repo).ok();
    }

    #[test]
    fn expanded_sidebar_workspace_numbers_follow_priority_visible_order() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one"), Workspace::test_new("two")];
        // A branch gives each entry a second (branch) row, where the jump number
        // renders under the dot.
        app.workspaces[0].cached_git_branch = Some("main".into());
        app.workspaces[1].cached_git_branch = Some("main".into());
        app.ensure_test_terminals();
        app.show_workspace_numbers = true;
        app.workspace_sort = crate::app::state::WorkspaceSort::Priority;

        // "two" (state index 1) is blocked, so it bubbles to visible position 0
        // and must take jump number 1; "one" falls to position 1 (number 2).
        let pane = app.workspaces[1].tabs[0].root_pane;
        let terminal_id = app.workspaces[1].tabs[0].panes[&pane]
            .attached_terminal_id
            .clone();
        let terminal = app.terminals.get_mut(&terminal_id).unwrap();
        terminal.detected_agent = Some(crate::detect::Agent::Claude);
        terminal.state = crate::detect::AgentState::Blocked;
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Navigate;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let top = app.view.workspace_card_areas[0].rect;
        let second = app.view.workspace_card_areas[1].rect;

        // Row 0 shows the name; the jump number sits under the dot on row 1
        // (column x+1 for a plain space) and follows the priority-visible order.
        assert!(
            buffer_row_text(buffer, top, top.y).contains("two"),
            "top row0: {:?}",
            buffer_row_text(buffer, top, top.y)
        );
        assert_eq!(buffer[(top.x + 1, top.y + 1)].symbol(), "1");
        assert_eq!(buffer[(second.x + 1, second.y + 1)].symbol(), "2");

        // The number label is styled with the palette default when no override.
        assert_eq!(
            buffer[(top.x + 1, top.y + 1)].style().fg,
            Some(app.palette.overlay0)
        );
    }

    #[test]
    fn expanded_sidebar_active_border_left_draws_bar_on_active_card_edge() {
        let mut app = crate::app::state::AppState::test_new();
        app.workspaces = vec![Workspace::test_new("one"), Workspace::test_new("two")];
        app.ensure_test_terminals();
        app.sidebar_active_border = crate::config::SidebarActiveBorderConfig::Left;
        app.active = Some(1);
        app.selected = 1;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let active_card = app.view.workspace_card_areas[1].rect;
        let bar_symbol = buffer[(active_card.x, active_card.y)].symbol().to_string();
        assert_eq!(bar_symbol, "│", "expected vertical bar on active card edge");

        let inactive_card = app.view.workspace_card_areas[0].rect;
        let inactive_symbol = buffer[(inactive_card.x, inactive_card.y)]
            .symbol()
            .to_string();
        assert_ne!(inactive_symbol, "│", "inactive card must not have a bar");
    }

    #[test]
    fn tab_bar_dims_auto_named_tabs_and_emphasizes_custom_tabs() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        let custom_tab = ws.test_add_tab(Some("logs"));
        ws.switch_tab(custom_tab);

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let auto_rect = app.view.tab_hit_areas[0];
        let custom_rect = app.view.tab_hit_areas[1];
        let auto_style = buffer[(auto_rect.x + 1, auto_rect.y)].style();
        let custom_style = buffer[(custom_rect.x + 1, custom_rect.y)].style();

        assert_eq!(auto_style.fg, Some(app.palette.overlay0));
        assert!(auto_style.add_modifier.contains(Modifier::DIM));
        assert_eq!(custom_style.fg, Some(app.palette.panel_bg));
        assert!(custom_style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn tab_bar_uses_surface_dim_when_panel_background_resets() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        let custom_tab = ws.test_add_tab(Some("logs"));
        ws.switch_tab(custom_tab);

        app.palette.panel_bg = Color::Reset;
        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
        let buffer = terminal.backend().buffer();

        let custom_rect = app.view.tab_hit_areas[1];
        let custom_style = buffer[(custom_rect.x + 1, custom_rect.y)].style();

        assert_eq!(custom_style.bg, Some(app.palette.accent));
        assert_eq!(custom_style.fg, Some(app.palette.surface_dim));
        assert!(custom_style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn new_tab_button_tracks_rightmost_tab_when_tabs_fit() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.test_add_tab(Some("logs"));

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;

        compute_view(&mut app, Rect::new(0, 0, 80, 20));

        let last_visible = app
            .view
            .tab_hit_areas
            .iter()
            .rev()
            .find(|rect| rect.width > 0)
            .copied()
            .expect("last visible tab");

        assert_eq!(
            app.view.new_tab_hit_area.x,
            last_visible.x + last_visible.width
        );
    }

    #[test]
    fn tab_bar_shows_scroll_controls_when_tabs_overflow() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        for name in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta"] {
            ws.test_add_tab(Some(name));
        }

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.tab_scroll_follow_active = false;
        app.tab_scroll = 2;

        compute_view(&mut app, Rect::new(0, 0, 65, 20));

        assert!(app.view.tab_scroll_left_hit_area.width > 0);
        assert!(app.view.tab_scroll_right_hit_area.width > 0);
        assert_eq!(app.view.tab_hit_areas[0].width, 0);
        assert_eq!(app.view.tab_hit_areas[1].width, 0);
        assert!(app.view.tab_hit_areas[2].width > 0);
        assert!(app.view.new_tab_hit_area.width > 0);

        let last_visible = app
            .view
            .tab_hit_areas
            .iter()
            .rev()
            .find(|rect| rect.width > 0)
            .copied()
            .expect("last visible tab");

        assert_eq!(
            app.view.tab_scroll_right_hit_area.x,
            last_visible.x + last_visible.width
        );
        assert_eq!(
            app.view.new_tab_hit_area.x,
            app.view.tab_scroll_right_hit_area.x + app.view.tab_scroll_right_hit_area.width
        );
    }

    #[test]
    fn tab_bar_clamps_manual_scroll_at_last_visible_tab() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        for name in [
            "one", "two", "three", "four", "five", "six", "seven", "eight",
        ] {
            ws.test_add_tab(Some(name));
        }

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.tab_scroll_follow_active = false;
        app.tab_scroll = usize::MAX;

        compute_view(&mut app, Rect::new(0, 0, 65, 20));

        let last_idx = app.workspaces[0].tabs.len() - 1;
        assert!(app.view.tab_hit_areas[last_idx].width > 0);
        let clamped_scroll = app.tab_scroll;

        app.scroll_tabs_right();

        assert_eq!(app.tab_scroll, clamped_scroll);
        assert!(app.view.tab_hit_areas[last_idx].width > 0);
    }

    #[test]
    fn pane_scrollbar_rect_uses_reserved_rightmost_column() {
        let info = PaneInfo {
            id: crate::layout::PaneId::from_raw(1),
            rect: Rect::new(0, 0, 12, 8),
            inner_rect: Rect::new(1, 1, 9, 6),
            scrollbar_rect: Some(Rect::new(10, 1, 1, 6)),
            borders: ratatui::widgets::Borders::ALL,
            is_focused: true,
        };

        assert_eq!(pane_scrollbar_rect(&info), Some(Rect::new(10, 1, 1, 6)));
    }

    #[tokio::test]
    async fn compute_view_reserves_terminal_column_when_pane_scrollbar_is_visible() {
        let mut app = crate::app::state::AppState::test_new();
        let mut ws = Workspace::test_new("test");
        let pane_id = ws.tabs[0].root_pane;
        ws.insert_test_runtime(
            pane_id,
            crate::terminal::TerminalRuntime::test_with_scrollback_bytes(
                12,
                4,
                4096,
                b"000000000000\r\n111111111111\r\n222222222222\r\n333333333333\r\n444444444444\r\n",
            ),
        );

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.selected = 0;

        compute_view(&mut app, Rect::new(0, 0, 40, 12));

        let info = app.view.pane_infos.first().expect("pane info");
        assert_eq!(info.inner_rect.width + 1, app.view.terminal_area.width);
        assert_eq!(
            info.scrollbar_rect,
            Some(Rect::new(
                info.inner_rect.x + info.inner_rect.width,
                info.inner_rect.y,
                1,
                info.inner_rect.height,
            ))
        );
    }

    #[test]
    fn scrollbar_stays_hidden_without_scrollback() {
        let metrics = crate::pane::ScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 0,
            viewport_rows: 5,
        };

        assert!(!should_show_scrollbar(metrics));
    }

    #[test]
    fn scrollbar_shows_with_scrollback() {
        let metrics = crate::pane::ScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 20,
            viewport_rows: 5,
        };

        assert!(should_show_scrollbar(metrics));
    }

    #[test]
    fn scrollbar_thumb_reaches_bottom_when_scrolled_to_bottom() {
        let metrics = crate::pane::ScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 20,
            viewport_rows: 5,
        };
        let track = Rect::new(9, 4, 1, 5);

        let thumb = scrollbar_thumb(metrics, track).expect("thumb");
        assert_eq!(thumb.top + thumb.len, track.y + track.height);
    }

    #[test]
    fn scrollbar_offset_mapping_hits_top_middle_and_bottom() {
        let metrics = crate::pane::ScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 20,
            viewport_rows: 5,
        };
        let track = Rect::new(9, 4, 1, 5);

        assert_eq!(scrollbar_offset_from_row(metrics, track, 4), 20);
        assert_eq!(scrollbar_offset_from_row(metrics, track, 6), 10);
        assert_eq!(scrollbar_offset_from_row(metrics, track, 8), 0);
    }

    #[test]
    fn dragging_from_current_thumb_row_preserves_offset() {
        let metrics = crate::pane::ScrollMetrics {
            offset_from_bottom: 7,
            max_offset_from_bottom: 20,
            viewport_rows: 5,
        };
        let track = Rect::new(9, 4, 1, 8);
        let thumb = scrollbar_thumb(metrics, track).expect("thumb");
        let row = thumb.top + thumb.len / 2;
        let grab = scrollbar_thumb_grab_offset(metrics, track, row).expect("grab");

        assert_eq!(scrollbar_offset_from_drag_row(metrics, track, row, grab), 7);
    }

    fn buffer_row_text(buffer: &ratatui::buffer::Buffer, area: Rect, row: u16) -> String {
        (area.x..area.x + area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn temp_git_repo(branch: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("herdr-ui-test-{unique}"));
        std::fs::create_dir_all(root.join(".git")).expect("create .git dir");
        std::fs::write(
            root.join(".git/HEAD"),
            format!("ref: refs/heads/{branch}\n"),
        )
        .expect("write HEAD");
        root
    }

    #[test]
    fn prefix_mode_renders_prefix_indicator() {
        let mut app = crate::app::state::AppState::test_new();
        app.mode = Mode::Prefix;
        app.view.terminal_area = ratatui::layout::Rect::new(0, 0, 60, 4);
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 4))
            .expect("test terminal");

        terminal
            .draw(|frame| render_prefix_overlay(&app, frame, app.view.terminal_area))
            .expect("draw prefix overlay");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("PREFIX"));
    }

    #[test]
    fn keybind_help_lists_the_pane_todo_panel_action() {
        let app = crate::app::state::AppState::test_new();
        let groups = keybind_help_groups(&app);
        let panes = groups
            .iter()
            .find(|(name, _)| *name == "panes")
            .expect("panes group")
            .1
            .clone();

        assert!(panes
            .iter()
            .any(|(key, label)| key == "prefix+ctrl+t" && label.as_ref() == "pane todos"));
    }

    #[test]
    fn keybind_help_shows_unset_for_the_add_pane_todo_action() {
        let app = crate::app::state::AppState::test_new();
        let groups = keybind_help_groups(&app);
        let panes = groups
            .iter()
            .find(|(name, _)| *name == "panes")
            .expect("panes group")
            .1
            .clone();

        assert!(
            panes
                .iter()
                .any(|(key, label)| key == "unset" && label.as_ref() == "add pane todo"),
            "an unbound action is still discoverable in the help panel"
        );
    }

    /// Spec: "every action introduced by this feature is listed". The edit
    /// modal's chords are fixed rather than `KeysConfig` actions, which is
    /// exactly why they need listing — nothing else advertises them, least of
    /// all the three that moved.
    #[test]
    fn keybind_help_lists_the_todo_editing_chords() {
        let app = crate::app::state::AppState::test_new();
        let groups = keybind_help_groups(&app);
        let modal = groups
            .iter()
            .find(|(name, _)| *name == "todo edit modal")
            .expect("todo edit modal group")
            .1
            .clone();
        let listed = |label: &str| modal.iter().any(|(_, entry)| entry.as_ref() == label);
        let key_for = |label: &str| {
            modal
                .iter()
                .find(|(_, entry)| entry.as_ref() == label)
                .map(|(key, _)| key.clone())
                .unwrap_or_default()
        };

        // The three that broke muscle memory.
        assert_eq!(key_for("save todo"), "ctrl+s / alt+enter");
        assert_eq!(key_for("toggle done"), "ctrl+t");
        assert_eq!(key_for("save and follow the link"), "ctrl+g");
        assert_eq!(key_for("kill to line end / start"), "ctrl+k / ctrl+u");
        assert_eq!(key_for("insert newline"), "enter");

        for action in [
            "line start / end",
            "character back / forward",
            "word back / forward",
            "delete forward",
            "kill word back",
            "yank last kill",
            "undo",
        ] {
            assert!(listed(action), "{action} is not discoverable");
        }

        let panel = groups
            .iter()
            .find(|(name, _)| *name == "pane todos")
            .expect("pane todos group")
            .1
            .clone();
        assert!(panel
            .iter()
            .any(|(key, label)| key == "a" && label.as_ref() == "add todo"));
    }

    #[test]
    fn keybind_help_shows_unset_for_optional_actions() {
        let app = crate::app::state::AppState::test_new();
        let groups = keybind_help_groups(&app);

        let workspace_tab = groups
            .iter()
            .find(|(name, _)| *name == "workspaces / tabs")
            .expect("workspace tab group")
            .1
            .clone();
        let panes = groups
            .iter()
            .find(|(name, _)| *name == "panes")
            .expect("panes group")
            .1
            .clone();

        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "previous workspace"));
        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "next workspace"));
        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "previous agent"));
        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "next agent"));
        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "focus agent 1-9"));
        assert!(workspace_tab
            .iter()
            .any(|(key, label)| key == "unset" && label.as_ref() == "switch workspace 1-9"));
        assert!(panes
            .iter()
            .any(|(key, label)| key == "prefix+h" && label.as_ref() == "focus pane left"));
        assert!(panes
            .iter()
            .any(|(key, label)| key == "prefix+j" && label.as_ref() == "focus pane down"));
        assert!(panes
            .iter()
            .any(|(key, label)| key == "prefix+k" && label.as_ref() == "focus pane up"));
        assert!(panes
            .iter()
            .any(|(key, label)| key == "prefix+l" && label.as_ref() == "focus pane right"));
    }

    #[test]
    fn keybind_help_shows_custom_command_descriptions() {
        let mut app = crate::app::state::AppState::test_new();
        app.keybinds.custom_commands = vec![
            crate::config::CustomCommandKeybind {
                bindings: crate::config::ActionKeybinds::prefix("alt+g"),
                label: "prefix+alt+g".to_string(),
                command: "lazygit".to_string(),
                action: crate::config::CustomCommandAction::Pane,
                description: Some("open lazygit".to_string()),
                width: None,
                height: None,
            },
            crate::config::CustomCommandKeybind {
                bindings: crate::config::ActionKeybinds::prefix("alt+h"),
                label: "prefix+alt+h".to_string(),
                command: "echo hello".to_string(),
                action: crate::config::CustomCommandAction::Shell,
                description: None,
                width: None,
                height: None,
            },
        ];

        let groups = keybind_help_groups(&app);
        let custom = groups
            .iter()
            .find(|(name, _)| *name == "custom")
            .expect("custom group")
            .1
            .clone();
        assert!(custom
            .iter()
            .any(|(key, label)| key == "prefix+alt+g" && label.as_ref() == "open lazygit"));
        assert!(custom
            .iter()
            .any(|(key, label)| key == "prefix+alt+h" && label.as_ref() == "custom command"));

        let rendered_help = keybind_help_lines(&app)
            .into_iter()
            .flat_map(|(_, line)| line.spans)
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");
        assert!(rendered_help.contains("open lazygit"));
        assert!(rendered_help.contains("custom command"));
    }

    #[test]
    fn keybind_help_compacts_multiple_indexed_ranges() {
        let config: crate::config::Config = toml::from_str(
            r#"
[keys]
switch_tab = ["prefix+1..9", "alt+1..9"]
switch_workspace = "ctrl+1..9"
"#,
        )
        .expect("config parses");

        let mut app = crate::app::state::AppState::test_new();
        app.keybinds = config.keybinds();

        let workspace_tab = keybind_help_groups(&app)
            .into_iter()
            .find(|(name, _)| *name == "workspaces / tabs")
            .expect("workspace tab group")
            .1;

        let switch_tab_key = workspace_tab
            .iter()
            .find(|(_, label)| label.as_ref() == "switch tab 1-9")
            .map(|(key, _)| key.as_str())
            .expect("switch tab help entry");
        let switch_workspace_key = workspace_tab
            .iter()
            .find(|(_, label)| label.as_ref() == "switch workspace 1-9")
            .map(|(key, _)| key.as_str())
            .expect("switch workspace help entry");

        assert_eq!(switch_tab_key, "prefix+1..9 / alt+1..9");
        assert_eq!(switch_workspace_key, "ctrl+1..9");
    }

    #[test]
    fn keybind_help_compacts_letter_ranges() {
        let config: crate::config::Config = toml::from_str(
            r#"
[keys]
focus_agent = ["prefix+alt+1..9", "prefix+alt+a..z"]
"#,
        )
        .expect("config parses");

        let mut app = crate::app::state::AppState::test_new();
        app.keybinds = config.keybinds();

        let workspace_tab = keybind_help_groups(&app)
            .into_iter()
            .find(|(name, _)| *name == "workspaces / tabs")
            .expect("workspace tab group")
            .1;
        let focus_agent_key = workspace_tab
            .iter()
            .find(|(_, label)| label.as_ref() == "focus agent 1-9")
            .map(|(key, _)| key.as_str())
            .expect("focus agent help entry");

        assert_eq!(focus_agent_key, "prefix+alt+1..9 / prefix+alt+a..z");
    }
}
