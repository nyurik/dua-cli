use super::widgets::{DisplayState, MainWindow};
use crate::{sorted_entries, traverse::Traversal, ByteFormat, WalkOptions, WalkResult};
use failure::Error;
use itertools::Itertools;
use petgraph::Direction;
use std::{io, path::PathBuf};
use termion::input::{Keys, TermReadEventsAndRaw};
use tui::widgets::Widget;
use tui::{backend::Backend, Terminal};

/// Options to configure how we display things
#[derive(Clone, Copy)]
pub struct DisplayOptions {
    pub byte_format: ByteFormat,
}

impl From<WalkOptions> for DisplayOptions {
    fn from(WalkOptions { byte_format, .. }: WalkOptions) -> Self {
        DisplayOptions { byte_format }
    }
}

/// State and methods representing the interactive disk usage analyser for the terminal
pub struct TerminalApp {
    pub traversal: Traversal,
    pub display: DisplayOptions,
    pub state: DisplayState,
}

enum CursorDirection {
    Down,
    Up,
}

impl TerminalApp {
    fn draw<B>(&self, terminal: &mut Terminal<B>) -> Result<(), Error>
    where
        B: Backend,
    {
        let Self {
            traversal,
            display,
            state,
        } = self;

        terminal.draw(|mut f| {
            let full_screen = f.size();
            MainWindow {
                traversal,
                display: *display,
                state: &state,
            }
            .render(&mut f, full_screen)
        })?;

        Ok(())
    }
    pub fn process_events<B, R>(
        &mut self,
        terminal: &mut Terminal<B>,
        keys: Keys<R>,
    ) -> Result<WalkResult, Error>
    where
        B: Backend,
        R: io::Read + TermReadEventsAndRaw,
    {
        use termion::event::Key::{Char, Ctrl};

        self.draw(terminal)?;
        for key in keys.filter_map(Result::ok) {
            match key {
                Char('u') => {
                    if let Some(parent_idx) = self
                        .traversal
                        .tree
                        .neighbors_directed(self.state.root, Direction::Incoming)
                        .next()
                    {
                        self.state.root = parent_idx;
                        self.state.selected =
                            sorted_entries(&self.traversal.tree, parent_idx, self.state.sorting)
                                .get(0)
                                .map(|(idx, _)| *idx);
                    }
                }
                Char('o') => self.enter_node(),
                Char('k') => self.change_vertical_index(CursorDirection::Up),
                Char('j') => self.change_vertical_index(CursorDirection::Down),
                Char('s') => self.state.sorting.toggle_size(),
                Ctrl('c') | Char('q') => break,
                _ => {}
            };
            self.draw(terminal)?;
        }
        Ok(WalkResult {
            num_errors: self.traversal.io_errors,
        })
    }

    fn enter_node(&mut self) -> () {
        if let Some(idx) = self.state.selected {
            let entries = sorted_entries(&self.traversal.tree, idx, self.state.sorting);
            if let Some((next_selection, _)) = entries.get(0) {
                self.state.root = idx;
                self.state.selected = Some(*next_selection);
            }
        }
    }

    fn change_vertical_index(&mut self, direction: CursorDirection) -> () {
        let entries = sorted_entries(&self.traversal.tree, self.state.root, self.state.sorting);
        let next_selected_pos = match self.state.selected {
            Some(ref selected) => entries
                .iter()
                .find_position(|(idx, _)| *idx == *selected)
                .map(|(idx, _)| match direction {
                    CursorDirection::Down => idx.saturating_add(1),
                    CursorDirection::Up => idx.saturating_sub(1),
                })
                .unwrap_or(0),
            None => 0,
        };
        self.state.selected = match entries.get(next_selected_pos) {
            Some((idx, _)) => Some(*idx),
            None => self.state.selected,
        };
    }

    pub fn initialize<B>(
        terminal: &mut Terminal<B>,
        options: WalkOptions,
        input: Vec<PathBuf>,
    ) -> Result<TerminalApp, Error>
    where
        B: Backend,
    {
        let display_options: DisplayOptions = options.clone().into();
        let traversal = Traversal::from_walk(options, input, move |traversal| {
            terminal.draw(|mut f| {
                let full_screen = f.size();
                let state = DisplayState {
                    root: traversal.root_index,
                    selected: None,
                    sorting: Default::default(),
                };
                MainWindow {
                    traversal,
                    display: display_options,
                    state: &state,
                }
                .render(&mut f, full_screen)
            })?;
            Ok(())
        })?;

        let sorting = Default::default();
        let root = traversal.root_index;
        let selected = sorted_entries(&traversal.tree, root, sorting)
            .get(0)
            .map(|(idx, _)| *idx);
        Ok(TerminalApp {
            state: DisplayState {
                root,
                selected,
                sorting,
            },
            display: display_options,
            traversal: traversal,
        })
    }
}