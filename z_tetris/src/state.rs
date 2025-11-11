use serde::{Deserialize, Serialize};

use crate::Field;

#[derive(Clone, Serialize, Deserialize)]
pub struct TetrisState {
    pub well: Field,
    pub preview: Field,
    pub game_over: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TetrisPairState {
    pub player: TetrisState,
    pub opponent: TetrisState,
}

impl TetrisPairState {
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.player, &mut self.opponent);
    }
}
