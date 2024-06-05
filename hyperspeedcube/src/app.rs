use std::path::PathBuf;
use std::sync::{Arc, Weak};

use hyperpuzzle::Puzzle;
use parking_lot::Mutex;

use crate::gfx::GraphicsState;
use crate::gui::PuzzleWidget;
use crate::preferences::Preferences;

pub struct App {
    pub(crate) gfx: Arc<GraphicsState>,

    pub(crate) prefs: Preferences,

    pub(crate) active_puzzle_view: Weak<Mutex<Option<PuzzleWidget>>>,
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, _initial_file: Option<PathBuf>) -> Self {
        Self {
            gfx: Arc::new(GraphicsState::new(
                cc.wgpu_render_state.as_ref().expect("no wgpu render state"),
            )),

            prefs: Preferences::load(None),

            active_puzzle_view: Weak::new(),
        }
    }

    /// Returns whether there is an active puzzle view. Do NOT rely on this
    /// being up-to-date; the result of this function may change by the time it
    /// returns.
    pub(crate) fn has_active_puzzle(&self) -> bool {
        self.active_puzzle_view.upgrade().is_some()
    }
    pub(crate) fn active_puzzle_type(&self) -> Option<Arc<Puzzle>> {
        self.with_active_puzzle_view(|puzzle_view| puzzle_view.puzzle())
    }
    pub(crate) fn with_active_puzzle_view<R>(
        &self,
        f: impl FnOnce(&mut PuzzleWidget) -> R,
    ) -> Option<R> {
        let active_puzzle_view = self.active_puzzle_view.upgrade()?;
        let mut puzzle_view_mutex_guard = active_puzzle_view.lock();
        Some(f(puzzle_view_mutex_guard.as_mut()?))
    }

    pub(crate) fn load_puzzle(&mut self, lib: &hyperpuzzle::Library, puzzle_id: &str) {
        match self.active_puzzle_view.upgrade() {
            Some(puzzle_view) => *puzzle_view.lock() = PuzzleWidget::new(lib, puzzle_id, &self.gfx),
            None => log::warn!("No active puzzle view"),
        }
    }
}
