mod frequency_regulator;
mod state;
mod term_render;
mod tetris;
mod tetris_pair;

use state::{TetrisPairState, TetrisState};
use term_render::{
    pad_block_right, render_block, AnsiTermStyle, GameFieldLeft, GameFieldPair, GameFieldRight,
    PlainTermStyle, PreviewField, TermCell, TermRender, TermStyle, WellField,
};
use tetris::{Action, Field, StepResult, Tetris};
use tetris_pair::{PlayerSide, TetrisPair};

fn main() {
    // TODO: Implement z_tetris application
}
