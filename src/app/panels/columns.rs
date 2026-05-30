use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    app::{
        App, DirColumn, SearchMode, display_home_relative, local_filtered_indices_with_query,
        parse_color, path_last_segment, scroll_start,
    },
    mascot::render_empty_dir_mascot,
};

impl App {
    pub(crate) fn draw_miller_columns(&self, frame: &mut Frame, area: Rect) {
        if self.search_mode == SearchMode::Global {
            self.draw_global_results(frame, area);
            return;
        }
        let max_columns = self.effective.max_columns.max(2);
        let (path_cols, outer_label) = self.visible_path_columns_fixed(max_columns);
        let outer = Block::default()
            .title(outer_label)
            .borders(Borders::ALL)
            .style(Style::default().fg(if self.sudo_mode {
                parse_color(&self.effective.theme_colors.vars.sudo_mode)
            } else {
                parse_color(&self.effective.theme_colors.vars.defult_panel_border)
            }));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if inner.width < 3 || inner.height < 3 || max_columns == 0 {
            return;
        }
        let ratio_total: u32 = self
            .effective
            .column_ratios
            .iter()
            .take(max_columns)
            .map(|&x| x.max(1) as u32)
            .sum();
        let constraints = self
            .effective
            .column_ratios
            .iter()
            .take(max_columns)
            .map(|&x| Constraint::Ratio(x.max(1) as u32, ratio_total.max(1)))
            .collect::<Vec<_>>();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(inner);

        let active_render_idx = self.effective.active_column.min(max_columns.saturating_sub(2));
        for idx in 0..max_columns {
            if idx == max_columns - 1 {
                if self.columns.len() <= 1 {
                    let p = Paragraph::new("").block(
                        Block::default().borders(Borders::ALL).style(
                            Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                        ),
                    );
                    frame.render_widget(p, chunks[idx]);
                } else if let Some(selected_dir) = self.selected_dir_path() {
                    let preview_col = DirColumn::from_path(
                        selected_dir.to_path_buf(),
                        &self.effective,
                        self.sudo_mode,
                        if self.sudo_password_input.is_empty() {
                            None
                        } else {
                            Some(self.sudo_password_input.as_str())
                        },
                    );
                    if preview_col.permission_denied {
                        let block = Block::default().borders(Borders::ALL).style(
                            Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                        );
                        let inner = block.inner(chunks[idx]);
                        frame.render_widget(block, chunks[idx]);
                        super::super::render_no_perms_mascot(frame, inner, &self.effective.theme_colors, &preview_col.path, self.sudo_mode);
                    } else if preview_col.entries.is_empty() {
                        let block = Block::default().borders(Borders::ALL).style(
                            Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                        );
                        let inner = block.inner(chunks[idx]);
                        frame.render_widget(block, chunks[idx]);
                        render_empty_dir_mascot(frame, inner, &self.effective.theme_colors);
                    } else {
                        let rows = preview_col
                            .entries
                            .iter()
                            .map(|e| {
                                let kind = if e.is_dir { "/" } else { "" };
                                ListItem::new(format!(" {}{}", e.name, kind)).style(
                                    Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_text)),
                                )
                            })
                            .collect::<Vec<_>>();
                        let list = List::new(rows).block(
                            Block::default()
                                .title(format!("/{}", path_last_segment(selected_dir)))
                                .borders(Borders::ALL)
                                .style(
                                    Style::default()
                                        .fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                                ),
                        );
                        frame.render_widget(list, chunks[idx]);
                    }
                } else {
                    let p = Paragraph::new("").block(
                        Block::default().borders(Borders::ALL).style(
                            Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                        ),
                    );
                    frame.render_widget(p, chunks[idx]);
                }
                continue;
            }

            let maybe_col = path_cols.get(idx).and_then(|c| *c);
            let Some(col) = maybe_col else {
                let p = Paragraph::new("").block(
                    Block::default()
                        .title("/")
                        .borders(Borders::ALL)
                        .style(
                            Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)),
                        ),
                );
                frame.render_widget(p, chunks[idx]);
                continue;
            };
            if col.permission_denied {
                let title = format!("/{}", path_last_segment(&col.path));
                let block = Block::default()
                    .title(Line::from(title).style(Style::default().fg(parse_color(
                        &self.effective.theme_colors.vars.defult_panel_label,
                    ))))
                    .borders(Borders::ALL)
                    .style(Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_border)));
                let inner = block.inner(chunks[idx]);
                frame.render_widget(block, chunks[idx]);
                super::super::render_no_perms_mascot(frame, inner, &self.effective.theme_colors, &col.path, self.sudo_mode);
                continue;
            }

            let is_focused_column = idx == active_render_idx;
            let col_theme = self.column_theme_for(idx);
            let is_locked_column = self
                .locked_column_path
                .as_ref()
                .map(|p| p == &col.path)
                .unwrap_or(true);
            let is_dimmed = self.selection_mode && !is_locked_column;
            let next_visible_path = if idx + 1 < path_cols.len() {
                path_cols[idx + 1].as_ref().map(|c| c.path.as_path())
            } else {
                self.selected_dir_path()
            };
            let filtered_indices = if is_focused_column && self.search_mode == SearchMode::Local {
                local_filtered_indices_with_query(&self.search_query, col)
            } else {
                (0..col.entries.len()).collect::<Vec<_>>()
            };
            let mode_highlight_active =
                self.sudo_mode || self.awaiting_bookmark_slot || self.selection_mode || self.search_mode != SearchMode::None;
            let mode_highlight_color = self.ui_mode_color_name();
            let viewport_height = chunks[idx].height.saturating_sub(2) as usize;
            let anchor_idx = if is_focused_column {
                filtered_indices.iter().position(|&x| x == col.selected).unwrap_or(0)
            } else {
                next_visible_path
                    .and_then(|p| col.entries.iter().position(|e| e.path == p))
                    .and_then(|abs| filtered_indices.iter().position(|&x| x == abs))
                    .unwrap_or(0)
            };
            let start = scroll_start(filtered_indices.len(), viewport_height, anchor_idx);
            let end = (start + viewport_height).min(filtered_indices.len());
            let mut rows = Vec::new();
            for filtered_row_idx in start..end {
                let absolute_idx = filtered_indices[filtered_row_idx];
                let entry = &col.entries[absolute_idx];
                let kind = if entry.is_dir { "/" } else { "" };
                let entry_fg = if idx == 0 || idx == 1 || idx == 3 {
                    parse_color(&self.effective.theme_colors.vars.defult_text)
                } else if entry.is_dir {
                    parse_color(&col_theme.dir)
                } else if entry.is_symlink {
                    parse_color(&col_theme.symlink)
                } else if entry.is_executable {
                    parse_color(&col_theme.executable)
                } else {
                    parse_color(&col_theme.file)
                };
                let base_fg = if is_focused_column {
                    if mode_highlight_active {
                        parse_color(mode_highlight_color)
                    } else {
                        entry_fg
                    }
                } else {
                    entry_fg
                };
                let mut item = ListItem::new(format!(" {}{}", entry.name, kind)).style(Style::default().fg(base_fg));
                let is_marked_selected = self.selection_set().contains(&entry.path);
                let should_highlight = (is_focused_column && absolute_idx == col.selected)
                    || next_visible_path.map(|p| entry.path == p).unwrap_or(false);
                if should_highlight {
                    let (bg, fg) = if mode_highlight_active {
                        (mode_highlight_color, &self.effective.theme_colors.vars.primary_bg)
                    } else if is_focused_column && absolute_idx == col.selected {
                        (&self.effective.theme_colors.vars.focused_dir_bg, &col_theme.focused_fg)
                    } else {
                        (&self.effective.theme_colors.vars.active_dir_bg, &col_theme.focused_fg)
                    };
                    item = item.style(Style::default().bg(parse_color(bg)).fg(parse_color(fg)));
                } else if is_marked_selected {
                    item = item.style(
                        Style::default()
                            .bg(parse_color(&col_theme.selected_bg))
                            .fg(if is_focused_column {
                                if mode_highlight_active {
                                    parse_color(mode_highlight_color)
                                } else {
                                    parse_color(&col_theme.selected_fg)
                                }
                            } else {
                                parse_color(&self.effective.theme_colors.vars.defult_text)
                            }),
                    );
                } else if is_dimmed {
                    item = item.style(
                        Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_text)),
                    );
                }
                rows.push(item);
            }
            if filtered_indices.is_empty() {
                let title = format!("/{}", path_last_segment(&col.path));
                let block = Block::default()
                    .title(Line::from(title).style(Style::default().fg(parse_color(
                        if is_focused_column && mode_highlight_active {
                            mode_highlight_color
                        } else if is_focused_column {
                            &col_theme.header
                        } else {
                            &self.effective.theme_colors.vars.defult_panel_label
                        },
                    ))))
                    .borders(Borders::ALL)
                    .style(Style::default().fg(parse_color(if is_focused_column {
                        if mode_highlight_active {
                            mode_highlight_color
                        } else {
                            &col_theme.border
                        }
                    } else {
                        &self.effective.theme_colors.vars.defult_panel_border
                    })));
                let inner = block.inner(chunks[idx]);
                frame.render_widget(block, chunks[idx]);
                render_empty_dir_mascot(frame, inner, &self.effective.theme_colors);
                continue;
            }
            let title = format!("/{}", path_last_segment(&col.path));
            let list = List::new(rows).block(
                Block::default()
                    .title(Line::from(title).style(Style::default().fg(parse_color(
                        if is_focused_column && mode_highlight_active {
                            mode_highlight_color
                        } else if is_focused_column {
                            &col_theme.header
                        } else {
                            &self.effective.theme_colors.vars.defult_panel_label
                        },
                    ))))
                    .borders(Borders::ALL)
                    .style(Style::default().fg(parse_color(if is_focused_column {
                        if mode_highlight_active {
                            mode_highlight_color
                        } else {
                            &col_theme.border
                        }
                    } else {
                        &self.effective.theme_colors.vars.defult_panel_border
                    }))),
            );
            frame.render_widget(list, chunks[idx]);
            if idx == active_render_idx && filtered_indices.len() > viewport_height && viewport_height > 0 {
                let mut state = ScrollbarState::new(filtered_indices.len()).position(start);
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_style(Style::default().fg(parse_color(&col_theme.scrollbar))),
                    chunks[idx],
                    &mut state,
                );
            }
        }
    }

    fn visible_path_columns_fixed(&self, max_columns: usize) -> (Vec<Option<&DirColumn>>, String) {
        let path_slots = max_columns.saturating_sub(1);
        let depth = self.columns.len();
        let visible_count = depth.min(path_slots);
        let start = depth.saturating_sub(visible_count);
        let mut cols = Vec::new();
        let pad = path_slots.saturating_sub(visible_count);
        for _ in 0..pad {
            cols.push(None);
        }
        for col in &self.columns[start..] {
            cols.push(Some(col));
        }

        let outer_label = if let Some(first_real) = self.columns.get(start) {
            first_real
                .path
                .parent()
                .map(display_home_relative)
                .unwrap_or_else(|| display_home_relative(&first_real.path))
        } else {
            display_home_relative(&self.current_dir)
        };
        (cols, outer_label)
    }
}
