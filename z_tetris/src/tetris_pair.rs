use crate::{
    state::TetrisPairState,
    tetris::{Action, StepResult, Tetris},
};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum PlayerSide {
    Player,
    Opponent,
}

pub struct TetrisPair {
    player: Tetris,
    opponent: Tetris,
    // The step is performed when both players have called step method
    // This is to prevent one player from getting an advantage by calling step more often
    step_player: bool,
    step_opponent: bool,
    step_divergence: usize,
}

impl TetrisPair {
    pub fn new(cols: usize, rows: usize) -> TetrisPair {
        TetrisPair {
            player: Tetris::new(cols, rows),
            opponent: Tetris::new(cols, rows),
            step_player: false,
            step_opponent: false,
            step_divergence: 0,
        }
    }

    pub fn rows(&self) -> usize {
        self.player.rows()
    }

    pub fn cols(&self) -> usize {
        self.player.cols()
    }

    pub fn set_fall_speed(&mut self, lines: usize, steps: usize) {
        self.player.set_fall_speed(lines, steps);
        self.opponent.set_fall_speed(lines, steps);
    }

    pub fn set_drop_speed(&mut self, lines: usize, steps: usize) {
        self.player.set_drop_speed(lines, steps);
        self.opponent.set_drop_speed(lines, steps);
    }

    pub fn set_line_remove_speed(&mut self, lines: usize, steps: usize) {
        self.player.set_line_remove_speed(lines, steps);
        self.opponent.set_line_remove_speed(lines, steps);
    }

    pub fn step(&mut self) -> (StepResult, StepResult) {
        self.step_player = false;
        self.step_opponent = false;
        let step_result_player = self.player.step();
        let step_result_opponent = self.opponent.step();
        if step_result_player == StepResult::LineRemoved {
            self.opponent.add_action(Action::BottomRefill);
        }
        if step_result_opponent == StepResult::LineRemoved {
            self.player.add_action(Action::BottomRefill);
        }
        (step_result_player, step_result_opponent)
    }

    /// Use this method when players have different control loops
    /// This guarantees that the game will run on frequiency of the slowest player
    pub fn step_player(&mut self, player: PlayerSide) -> usize {
        match player {
            PlayerSide::Player => self.step_player = true,
            PlayerSide::Opponent => self.step_opponent = true,
        }
        if self.step_player && self.step_opponent {
            self.step();
            self.step_divergence = 0;
        } else {
            self.step_divergence += 1;
        }
        self.step_divergence
    }

    pub fn add_player_action(&mut self, player: PlayerSide, action: Action) {
        match player {
            PlayerSide::Player => self.player.add_action(action),
            PlayerSide::Opponent => self.opponent.add_action(action),
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.player.is_game_over() || self.opponent.is_game_over()
    }

    pub fn get_state(&self) -> TetrisPairState {
        TetrisPairState {
            player: self.player.get_state(),
            opponent: self.opponent.get_state(),
        }
    }
}
