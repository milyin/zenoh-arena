use crate::{frequency_regulator::FrequencyRegulator, state::TetrisState};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellType {
    Empty = 0,
    Blasted,
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

impl CellType {
    pub fn new_random() -> CellType {
        match rand::random::<u8>() % 7 {
            0 => CellType::I,
            1 => CellType::J,
            2 => CellType::L,
            3 => CellType::O,
            4 => CellType::S,
            5 => CellType::T,
            6 => CellType::Z,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    // Rotate left
    pub fn rotate_left(&self) -> Rotation {
        match self {
            Rotation::R0 => Rotation::R270,
            Rotation::R90 => Rotation::R0,
            Rotation::R180 => Rotation::R90,
            Rotation::R270 => Rotation::R180,
        }
    }
    // Rotate right
    pub fn rotate_right(&self) -> Rotation {
        match self {
            Rotation::R0 => Rotation::R90,
            Rotation::R90 => Rotation::R180,
            Rotation::R180 => Rotation::R270,
            Rotation::R270 => Rotation::R0,
        }
    }
}

// Implement + operator for rotation
impl std::ops::Add<Rotation> for Rotation {
    type Output = Rotation;
    fn add(self, other: Rotation) -> Rotation {
        match other {
            Rotation::R0 => self,
            Rotation::R90 => self.rotate_right(),
            Rotation::R180 => self.rotate_right().rotate_right(),
            Rotation::R270 => self.rotate_left(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
struct TetrominoMatrix {
    matrix: [[bool; 4]; 4],
    width: usize,
    height: usize,
}

impl TetrominoMatrix {
    // Get width of tetromino matrix considering rotation
    pub fn get_width(&self, rotation: &Rotation) -> usize {
        // Return width of tetromino matrix considering rotation
        match rotation {
            Rotation::R0 => self.width,
            Rotation::R90 => self.height,
            Rotation::R180 => self.width,
            Rotation::R270 => self.height,
        }
    }
    // Get height of tetromino matrix considering rotation
    pub fn get_height(&self, rotation: &Rotation) -> usize {
        // Return height of tetromino matrix considering rotation
        match rotation {
            Rotation::R0 => self.height,
            Rotation::R90 => self.width,
            Rotation::R180 => self.height,
            Rotation::R270 => self.width,
        }
    }
    // Get cell value of tetromino matrix considering rotation
    pub fn get_cell(&self, x: usize, y: usize, rotation: &Rotation) -> bool {
        // Return cell value of tetromino matrix considering rotation
        match rotation {
            Rotation::R0 => self.matrix[y][x],
            Rotation::R90 => self.matrix[self.height - x - 1][y],
            Rotation::R180 => self.matrix[self.height - y - 1][self.width - x - 1],
            Rotation::R270 => self.matrix[x][self.width - y - 1],
        }
    }
}

// Constant tetromino matrix for I in R0 rotation, matrix is always 4x4
const TETROMINO_I_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [true, true, true, true],
        [false, false, false, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 4,
    height: 1,
};

// Constant tetromino matrix for J in R0 rotation, matrix is always 4x4
const TETROMINO_J_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [true, false, false, false],
        [true, true, true, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 3,
    height: 2,
};

// Constant tetromino matrix for L in R0 rotation, matrix is always 4x4
const TETROMINO_L_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [false, false, true, false],
        [true, true, true, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 3,
    height: 2,
};

// Constant tetromino matrix for O in R0 rotation, matrix is always 4x4
const TETROMINO_O_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [true, true, false, false],
        [true, true, false, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 2,
    height: 2,
};

// Constant tetromino matrix for S in R0 rotation, matrix is always 4x4
const TETROMINO_S_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [false, true, true, false],
        [true, true, false, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 3,
    height: 2,
};

// Constant tetromino matrix for T in R0 rotation, matrix is always 4x4
const TETROMINO_T_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [false, true, false, false],
        [true, true, true, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 3,
    height: 2,
};

// Constant tetromino matrix for Z in R0 rotation, matrix is always 4x4
const TETROMINO_Z_R0: TetrominoMatrix = TetrominoMatrix {
    matrix: [
        [true, true, false, false],
        [false, true, true, false],
        [false, false, false, false],
        [false, false, false, false],
    ],
    width: 3,
    height: 2,
};

// Get tetromino matrix by tetromino type
fn get_tetromino_matrix(tetromino_type: &TetrominoType) -> &TetrominoMatrix {
    // Return tetromino matrix by tetromino type
    match tetromino_type {
        TetrominoType::I => &TETROMINO_I_R0,
        TetrominoType::J => &TETROMINO_J_R0,
        TetrominoType::L => &TETROMINO_L_R0,
        TetrominoType::O => &TETROMINO_O_R0,
        TetrominoType::S => &TETROMINO_S_R0,
        TetrominoType::T => &TETROMINO_T_R0,
        TetrominoType::Z => &TETROMINO_Z_R0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TetrominoType {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

impl TetrominoType {
    // new method returns new tetromino type
    pub fn new_random() -> Self {
        // Create new tetromino type
        // Create random number between 0 and 6
        let random_number = rand::random::<u32>() % 7;
        // Return new tetromino type
        match random_number {
            0 => TetrominoType::I,
            1 => TetrominoType::J,
            2 => TetrominoType::L,
            3 => TetrominoType::O,
            4 => TetrominoType::S,
            5 => TetrominoType::T,
            6 => TetrominoType::Z,
            _ => TetrominoType::I,
        }
    }

    // Get tetromino width depending on rotation
    pub fn get_width(&self, rotation: &Rotation) -> usize {
        // Return tetromino width depending on rotation
        get_tetromino_matrix(self).get_width(rotation)
    }

    // Get tetromino height depending on rotation
    pub fn get_height(&self, rotation: &Rotation) -> usize {
        // Return tetromino height depending on rotation
        get_tetromino_matrix(self).get_height(rotation)
    }

    // Get cell value depending on rotation
    pub fn get_cell(&self, x: usize, y: usize, rotation: &Rotation) -> bool {
        // Return cell value depending on rotation
        get_tetromino_matrix(self).get_cell(x, y, rotation)
    }

    // Get cell type corresponding to tetromino type
    pub fn get_cell_type(&self) -> CellType {
        // Return cell type corresponding to tetromino type
        match self {
            TetrominoType::I => CellType::I,
            TetrominoType::J => CellType::J,
            TetrominoType::L => CellType::L,
            TetrominoType::O => CellType::O,
            TetrominoType::S => CellType::S,
            TetrominoType::T => CellType::T,
            TetrominoType::Z => CellType::Z,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Tetromino {
    // Tetromino type
    tetromino_type: TetrominoType,
    // Tetromino rotation
    rotation: Rotation,
    // Tetromino position
    x: isize,
    y: isize,
}

impl Tetromino {
    // new method accepts TetrominoType and returns new tetromino
    pub fn new(tetromino_type: TetrominoType, rotation: Rotation, x: isize, y: isize) -> Self {
        // Create new tetromino
        Tetromino {
            tetromino_type,
            rotation,
            x: x,
            y: y,
        }
    }

    // Check if tetromino intersects with field borders or other tetrominos
    pub fn intersects(&self, field: &Field) -> bool {
        // Check if tetromino intersects with field borders or other tetrominos
        // Check if tetromino position is positive, otherwise it intersects with field borders
        let x = if self.x >= 0 {
            self.x as usize
        } else {
            return true;
        };
        let y = if self.y >= 0 {
            self.y as usize
        } else {
            return true;
        };
        // Get tetromino width and height
        let width = self.tetromino_type.get_width(&self.rotation);
        let height = self.tetromino_type.get_height(&self.rotation);
        // Check if tetromino intersects with field borders
        if x + width > field.cols() || y + height > field.rows() {
            return true;
        }
        // Check if tetromino intersects with other tetrominos
        for cell_y in 0..height {
            for cell_x in 0..width {
                if self.tetromino_type.get_cell(cell_x, cell_y, &self.rotation)
                    && field.get_cell(x + cell_x, y + cell_y) != CellType::Empty
                {
                    return true;
                }
            }
        }
        // Return false if tetromino does not intersect with field borders or other tetrominos
        false
    }

    // Draw tetromino on field. If tetromino intersects with field borders, draw it partially.
    // I.e for any cell position check is it inside field borders and if it is, draw it.
    pub fn draw(&self, field: &mut Field) {
        // Draw tetromino on field
        // Get tetromino width and height
        let width = self.tetromino_type.get_width(&self.rotation);
        let height = self.tetromino_type.get_height(&self.rotation);
        let cell_type = self.tetromino_type.get_cell_type();
        // Draw tetromino on field
        for cell_y in 0..height {
            for cell_x in 0..width {
                if self.tetromino_type.get_cell(cell_x, cell_y, &self.rotation) {
                    // Get cell position. Use isize type to avoid overflow
                    // Check resulting positoins are positive and less than field borders
                    let x = self.x + cell_x as isize;
                    let y = self.y + cell_y as isize;
                    if x >= 0 && x < field.cols() as isize && y >= 0 && y < field.rows() as isize {
                        field.set_cell(x as usize, y as usize, cell_type);
                    }
                }
            }
        }
    }
}

// Enum with all possible user actions
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Action {
    MoveLeft,
    MoveRight,
    MoveDown,
    RotateLeft,
    RotateRight,
    Drop,
    BottomRefill,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Field {
    // Field width
    cols: usize,
    // Field height
    rows: usize,
    // Field cells
    cells: Vec<Vec<CellType>>,
}

impl Field {
    pub fn new(cols: usize, rows: usize) -> Self {
        // Create new field
        Field {
            cols,
            rows,
            cells: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn cols(&self) -> usize {
        // Return field width
        self.cols
    }

    pub fn rows(&self) -> usize {
        // Return field height
        self.rows
    }

    pub fn set_cell(&mut self, x: usize, y: usize, cell_type: CellType) {
        if x >= self.cols || y >= self.rows {
            return;
        }
        if self.cells.len() <= y {
            self.cells.resize(y + 1, Vec::new());
        }
        if self.cells[y].len() <= x {
            self.cells[y].resize(x + 1, CellType::Empty);
        }
        self.cells[y][x] = cell_type;
    }

    pub fn get_cell(&self, x: usize, y: usize) -> CellType {
        // Get cell value
        self.cells
            .get(y)
            .and_then(|row| row.get(x))
            .cloned()
            .unwrap_or(CellType::Empty)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub enum StepResult {
    // Nothing happened
    None,
    // Action was performed
    ActionPerformed(Action, bool),
    // Line removed
    LineRemoved,
    // Game over
    GameOver,
}

pub struct Tetris {
    // Game field size
    cols: usize,
    rows: usize,
    // Game over flag
    game_over: bool,
    // Game field
    well: Field,
    // Preview field
    preview: Field,
    // Current tetromino
    current: Option<Tetromino>,
    // Next tetromino
    next: TetrominoType,
    // User actions queue
    actions: VecDeque<Action>,
    // Drop state
    drop: bool,
    // Game speed
    fall_speed: FrequencyRegulator,
    // Drop speed
    drop_speed: FrequencyRegulator,
    // Blasting speed
    line_remove_speed: FrequencyRegulator,
    // Delay before line shifting
    line_remove_delay: Option<usize>,
    // Game score
    _score: usize,
}

impl Tetris {
    pub fn new(cols: usize, rows: usize) -> Self {
        // Create new tetris game
        // Create game field, functional style
        let well = Field::new(cols, rows);

        // Create preview field, functional style
        let mut preview = Field::new(4, 4);

        // Set next tetromino type
        let next = Self::create_next_tetromino_type(&mut preview);

        // Create user actions queue
        let actions = VecDeque::new();

        // Set game over flag
        let game_over = false;

        // Score
        let _score = 0;

        // Create new tetris game
        Tetris {
            cols,
            rows,
            game_over,
            well,
            preview,
            current: None,
            next,
            actions,
            drop: false,
            fall_speed: FrequencyRegulator::new(1, 100),
            drop_speed: FrequencyRegulator::new(1, 10),
            line_remove_speed: FrequencyRegulator::new(1, 3),
            line_remove_delay: None,
            _score,
        }
    }

    pub fn set_fall_speed(&mut self, lines: usize, steps: usize) {
        self.fall_speed = FrequencyRegulator::new(lines, steps);
    }

    pub fn set_drop_speed(&mut self, lines: usize, steps: usize) {
        self.drop_speed = FrequencyRegulator::new(lines, steps);
    }

    pub fn set_line_remove_speed(&mut self, lines: usize, steps: usize) {
        self.line_remove_speed = FrequencyRegulator::new(lines, steps);
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    // Add user action to actions queue
    pub fn add_action(&mut self, action: Action) {
        self.actions.push_back(action);
    }

    // Process single user action
    pub fn step(&mut self) -> StepResult {
        if self.game_over {
            return StepResult::GameOver;
        }

        if let Some(ref mut delay) = self.line_remove_delay {
            if *delay > 0 {
                *delay -= 1;
                return StepResult::None;
            } else {
                self.line_remove_delay = None;
            }
        }
        for _ in 0..self.line_remove_speed.step() {
            if self.current.is_none() {
                if self.remove_top_blasted_line() {
                    return StepResult::LineRemoved;
                } else {
                    if !self.place_next_tetromino() {
                        self.game_over = true;
                        return StepResult::GameOver;
                    }
                }
            }
        }

        if self.drop {
            for _ in 0..self.drop_speed.step() {
                self.actions.push_back(Action::MoveDown);
            }
        } else {
            for _ in 0..self.fall_speed.step() {
                self.actions.push_back(Action::MoveDown);
            }
        }

        let Some(action) = self.actions.pop_front() else {
            return StepResult::None;
        };
        let succeed = match action {
            Action::MoveLeft => self.move_left(),
            Action::MoveRight => self.move_right(),
            Action::MoveDown => self.move_down(),
            Action::RotateLeft => self.rotate_left(),
            Action::RotateRight => self.rotate_right(),
            Action::Drop => self.drop(),
            Action::BottomRefill => self.bottom_refill(),
        };
        // Move down is special case. If it fails, fix current tetromino and blast full lines
        if !succeed && action == Action::MoveDown {
            self.fix_current_figure();
            self.blast_full_lines();
            self.actions.clear();
            self.line_remove_delay = Some(10); // Wait 10 ticks before placing next tetromino to show blast animation
        }
        return StepResult::ActionPerformed(action, succeed);
    }

    // Create next tetromino type and draw it on preview field
    fn create_next_tetromino_type(preview: &mut Field) -> TetrominoType {
        // Create next tetromino and draw it on preview field
        // Get next tetromino type
        let tetromino_type = TetrominoType::new_random();
        // Create new tetromino
        let tetromino = Tetromino::new(tetromino_type, Rotation::R0, 0, 0);
        // Draw tetromino on preview field
        preview.clear();
        tetromino.draw(preview);
        // Get tetromino
        tetromino_type
    }

    pub fn get_field(&self) -> &Field {
        &self.well
    }

    pub fn get_preview(&self) -> &Field {
        &self.preview
    }

    pub fn get_current(&self) -> &Option<Tetromino> {
        &self.current
    }

    pub fn get_next(&self) -> &TetrominoType {
        &self.next
    }

    // Place new tetromino on the field. Return false if it's impossible to place new tetromino
    pub fn place_next_tetromino(&mut self) -> bool {
        // Create new tetromino
        let new_tetromino = Tetromino::new(self.next, Rotation::R0, self.cols as isize / 2 - 2, 0);

        // Check if new tetromino intersects with field borders or other tetrominos
        if new_tetromino.intersects(&self.well) {
            return false;
        }
        // Set new tetromino as current
        self.current = Some(new_tetromino);

        // Set next tetromino type and draw it on preview field
        self.next = Self::create_next_tetromino_type(&mut self.preview);

        // Clear drop flag
        self.drop = false;

        // Return true if new tetromino was placed on the field
        true
    }

    // Change position and rotation of current tetromino, if it's possible
    pub fn change_current_tetromino(&mut self, x: isize, y: isize, rotation: Rotation) -> bool {
        // Change position and rotation of current tetromino, if it's possible
        // Check if current tetromino exists
        let Some(current) = &mut self.current else {
            return false;
        };

        // Create new tetromino
        let new_tetromino = Tetromino::new(
            current.tetromino_type,
            current.rotation + rotation,
            current.x as isize + x,
            current.y as isize + y,
        );
        // Check if new tetromino intersects with field borders or other tetrominos
        if new_tetromino.intersects(&self.well) {
            return false;
        }
        *current = new_tetromino;
        return true;
    }

    // Move current tetromino down, if it's possible
    pub fn move_down(&mut self) -> bool {
        // Move current tetromino down, if it's possible
        self.change_current_tetromino(0, 1, Rotation::R0)
    }

    // Move current tetromino left, if it's possible
    pub fn move_left(&mut self) -> bool {
        // Move current tetromino left, if it's possible
        self.change_current_tetromino(-1, 0, Rotation::R0)
    }

    // Move current tetromino right, if it's possible
    pub fn move_right(&mut self) -> bool {
        // Move current tetromino right, if it's possible
        self.change_current_tetromino(1, 0, Rotation::R0)
    }

    // Rotate current tetromino left, if it's possible
    pub fn rotate_left(&mut self) -> bool {
        // Rotate current tetromino left, if it's possible
        self.change_current_tetromino(0, 0, Rotation::R270)
    }

    // Rotate current tetromino right, if it's possible
    pub fn rotate_right(&mut self) -> bool {
        // Rotate current tetromino right, if it's possible
        self.change_current_tetromino(0, 0, Rotation::R90)
    }

    // Set drop flag
    pub fn drop(&mut self) -> bool {
        // Set drop flag
        self.drop = true;
        true
    }

    // Push all lines up and fill bottom line with random cells with probability of filled cell = 0.5
    pub fn bottom_refill(&mut self) -> bool {
        // Push all lines up
        for y in 1..self.rows {
            for x in 0..self.cols {
                self.well.set_cell(x, y - 1, self.well.get_cell(x, y));
            }
        }
        // Fill bottom line with random cells with probability of filled cell = 0.3
        for x in 0..self.cols {
            let cell_type = if rand::random::<f32>() < 0.5 {
                CellType::new_random()
            } else {
                CellType::Empty
            };
            self.well.set_cell(x, self.rows - 1, cell_type);
        }
        true
    }

    // Draw current tetromino on the field
    pub fn fix_current_figure(&mut self) {
        // Draw current tetromino on the field
        // Check if current tetromino exists
        if let Some(current) = self.current.take() {
            // Draw current tetromino on the field
            current.draw(&mut self.well);
        }
    }

    // Blasts full lines and returns true if there were full lines
    fn blast_full_lines(&mut self) -> bool {
        // Iterate over all lines
        // If line is full, replace it's Empty cells to Blasted cells and set return value to true
        let mut full_lines = false;
        for y in 0..self.rows {
            let mut full_line = true;
            for x in 0..self.cols {
                if self.well.get_cell(x, y) == CellType::Empty {
                    full_line = false;
                    break;
                }
            }
            if full_line {
                full_lines = true;
                for x in 0..self.cols {
                    self.well.set_cell(x, y, CellType::Blasted);
                }
            }
        }
        full_lines
    }

    // Find topmost blasted line and shift all lines above it down to one line
    // Line is blasted if it's first cell is Blasted
    // Return false if there are no blasted lines
    fn remove_top_blasted_line(&mut self) -> bool {
        // Find topmost blasted line
        let mut top_blasted_line = None;
        for y in 0..self.rows {
            if self.well.get_cell(0, y) == CellType::Blasted {
                top_blasted_line = Some(y);
                break;
            }
        }
        // If there are no blasted lines, return false
        let Some(top_blasted_line) = top_blasted_line else {
            return false;
        };
        // Shift all lines above topmost blasted line down to one line
        for y in (0..top_blasted_line).rev() {
            for x in 0..self.cols {
                self.well.set_cell(x, y + 1, self.well.get_cell(x, y));
            }
        }
        // Fill topmost line with Empty cells
        for x in 0..self.cols {
            self.well.set_cell(x, 0, CellType::Empty);
        }
        // Return true if there were blasted lines
        true
    }

    // get game state for serialization
    pub fn get_state(&self) -> TetrisState {
        let mut well = self.well.clone();
        // draw current tetromino on the field
        if let Some(current) = &self.current {
            current.draw(&mut well);
        }
        let preview = self.preview.clone();
        TetrisState {
            well,
            preview,
            game_over: self.game_over,
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.game_over
    }
}
