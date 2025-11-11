use crate::{
    state::{TetrisPairState, TetrisState},
    tetris::CellType,
    Field,
};

#[derive(Clone, PartialEq)]
pub enum TermCell {
    FieldCell(CellType),
    BorderVertical,
    BorderHorizontal,
    BorderTopLeft,
    BorderTopRight,
    BorderBottomLeft,
    BorderBottomRight,
    Space,
    Message(String),
}

pub trait TermStyle {
    fn display<'a>(&self, cell: &'a TermCell) -> &'a str;
    fn width(&self, cell: &TermCell) -> usize;
}

pub trait TermRender {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>>;
    fn render(&self, style: &impl TermStyle) -> Vec<String> {
        let mut lines = Vec::new();
        for row in self.output(style) {
            let mut line = String::new();
            for cell in &row {
                line.push_str(style.display(cell));
            }
            lines.push(line);
        }
        lines
    }
}

// Make all lines in block the same width by padding with TermCell::Space
pub fn pad_block_right(block: &mut Vec<Vec<TermCell>>, style: &impl TermStyle) {
    // Requite that the width of TermCell::Space display is 1
    assert_eq!(style.width(&TermCell::Space), 1);
    // calculate width of each line of the block and the maximum width
    let mut widths = Vec::new();
    let mut width = 0;
    for row in block.iter() {
        let mut line_width = 0;
        for cell in row {
            line_width += style.width(cell);
        }
        widths.push(line_width);
        width = width.max(line_width);
    }
    // Pad all lines to the same width with 'cell'
    for (row, line_width) in block.iter_mut().zip(widths.iter()) {
        let padding = width - line_width;
        for _ in 0..padding {
            row.push(TermCell::Space);
        }
    }
}

pub fn render_block(block: &impl TermRender, style: &impl TermStyle) -> Vec<String> {
    let mut lines = Vec::new();
    for row in block.output(style) {
        let mut line = String::new();
        for cell in &row {
            line.push_str(style.display(cell));
        }
        lines.push(line);
    }
    lines
}

pub struct PlainTermStyle;

impl TermStyle for PlainTermStyle {
    fn display<'a>(&self, cell: &'a TermCell) -> &'a str {
        match cell {
            TermCell::FieldCell(CellType::Empty) => "  ",
            TermCell::FieldCell(CellType::Blasted) => "**",
            TermCell::FieldCell(_) => "[]",
            TermCell::BorderVertical => "|",
            TermCell::BorderTopLeft => "+",
            TermCell::BorderTopRight => "+",
            TermCell::BorderBottomLeft => "+",
            TermCell::BorderHorizontal => "--",
            TermCell::BorderBottomRight => "+",
            TermCell::Space => " ",
            TermCell::Message(s) => s.as_str(),
        }
    }
    fn width(&self, cell: &TermCell) -> usize {
        match cell {
            TermCell::FieldCell(_) => 2,
            TermCell::BorderVertical => 1,
            TermCell::BorderHorizontal => 2,
            TermCell::BorderTopLeft
            | TermCell::BorderTopRight
            | TermCell::BorderBottomLeft
            | TermCell::BorderBottomRight => 1,
            TermCell::Space => 1,
            TermCell::Message(s) => s.len(),
        }
    }
}

pub struct AnsiTermStyle;

impl TermStyle for AnsiTermStyle {
    fn display<'a>(&self, cell: &'a TermCell) -> &'a str {
        match cell {
            TermCell::FieldCell(CellType::Empty) => "\x1b[0m  ",
            TermCell::FieldCell(CellType::Blasted) => "\x1b[0;31m**",
            TermCell::FieldCell(CellType::I) => "\x1b[0;34m[]",
            TermCell::FieldCell(CellType::J) => "\x1b[0;32m[]",
            TermCell::FieldCell(CellType::L) => "\x1b[0;33m[]",
            TermCell::FieldCell(CellType::O) => "\x1b[0;35m[]",
            TermCell::FieldCell(CellType::S) => "\x1b[0;36m[]",
            TermCell::FieldCell(CellType::T) => "\x1b[0;37m[]",
            TermCell::FieldCell(CellType::Z) => "\x1b[0;31m[]",
            TermCell::BorderVertical => "\x1b[0m│",
            TermCell::BorderTopLeft => "\x1b[0m┌",
            TermCell::BorderTopRight => "\x1b[0m┐",
            TermCell::BorderBottomLeft => "\x1b[0m└",
            TermCell::BorderHorizontal => "\x1b[0m──",
            TermCell::BorderBottomRight => "\x1b[0m┘",
            TermCell::Space => " ",
            TermCell::Message(s) => s.as_str(),
        }
    }
    fn width(&self, cell: &TermCell) -> usize {
        match cell {
            TermCell::FieldCell(_) => 2,
            TermCell::BorderVertical => 1,
            TermCell::BorderHorizontal => 2,
            TermCell::BorderTopLeft
            | TermCell::BorderTopRight
            | TermCell::BorderBottomLeft
            | TermCell::BorderBottomRight => 1,
            TermCell::Space => 1,
            TermCell::Message(s) => s.len(),
        }
    }
}

impl TermRender for Field {
    fn output(&self, _style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = Vec::new();
        for row in 0..self.rows() {
            let mut line = Vec::new();
            for col in 0..self.cols() {
                let cell = self.get_cell(col, row);
                line.push(TermCell::FieldCell(cell));
            }
            lines.push(line);
        }
        lines
    }
}

pub struct WellField {
    field: Field,
    game_over: bool,
}

impl WellField {
    pub fn new(field: Field, game_over: bool) -> Self {
        Self { field, game_over }
    }
}

impl TermRender for WellField {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.field.output(style);
        if self.game_over {
            // Find middle line of the field
            let middle = lines.len() / 2;
            // Replace line with message
            let message = TermCell::Message("     Game Over".to_string());
            lines[middle] = vec![message];
            // Add padding
            // TODO: Center the message
            pad_block_right(&mut lines, style);
        }

        for line in &mut lines {
            line.insert(0, TermCell::BorderVertical);
            line.push(TermCell::BorderVertical);
        }
        let mut line = Vec::new();
        line.push(TermCell::BorderBottomLeft);
        for _ in 0..self.field.cols() {
            line.push(TermCell::BorderHorizontal);
        }
        line.push(TermCell::BorderBottomRight);
        lines.push(line);
        lines
    }
}

pub struct PreviewField(Field);

impl TermRender for PreviewField {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.0.output(style);
        for line in &mut lines {
            line.insert(0, TermCell::BorderVertical);
            line.push(TermCell::BorderVertical);
        }

        let mut line = Vec::new();
        line.push(TermCell::BorderTopLeft);
        for _ in 0..self.0.cols() {
            line.push(TermCell::BorderHorizontal);
        }
        line.push(TermCell::BorderTopRight);
        lines.insert(0, line);

        let mut line = Vec::new();
        line.push(TermCell::BorderBottomLeft);
        for _ in 0..self.0.cols() {
            line.push(TermCell::BorderHorizontal);
        }
        line.push(TermCell::BorderBottomRight);
        lines.push(line);

        lines
    }
}

pub struct GameFieldLeft {
    well: WellField,
    preview: PreviewField,
    text: Vec<String>,
}

impl GameFieldLeft {
    fn new(state: TetrisState, text: Vec<String>) -> Self {
        let well = WellField::new(state.well, state.game_over);
        let preview = PreviewField(state.preview);
        Self { well, preview, text }
    }
}

impl TermRender for GameFieldLeft {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.well.output(style);
        let mut preview_block = self.preview.output(style);
        // Append empty line and player name after preview block
        preview_block.push(Vec::new());
        preview_block.extend(self.text.iter().map(|s| vec![TermCell::Message(s.clone())]));
        // Append preview lines to well lines, padding with TermCell::Space
        // Preview is always shorter than well
        for (well_line, mut preview_line) in lines.iter_mut().zip(preview_block.into_iter()) {
            well_line.push(TermCell::Space);
            well_line.append(&mut preview_line);
        }
        pad_block_right(&mut lines, style);
        lines
    }
}

pub struct GameFieldRight {
    well: WellField,
    preview: PreviewField,
    text: Vec<String>,
}

impl GameFieldRight {
    fn new(state: TetrisState, text: Vec<String>) -> Self {
        let well = WellField::new(state.well, state.game_over);
        let preview = PreviewField(state.preview);
        Self { well, preview, text }
    }
}

impl TermRender for GameFieldRight {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.preview.output(style);
        let well_block = self.well.output(style);
        // Append empty line and text after preview block
        lines.push(Vec::new());
        lines.extend(self.text.iter().map(|s| vec![TermCell::Message(s.clone())]));
        // extend height of lines to the height of well_block and then pad it with TermCell::Space
        // Preview is always shorter than well
        lines.resize(well_block.len(), Vec::new());
        pad_block_right(&mut lines, style);
        // Append well lines to preview lines, padding with TermCell::Space
        for (preview_line, mut well_line) in lines.iter_mut().zip(well_block.into_iter()) {
            preview_line.push(TermCell::Space);
            preview_line.append(&mut well_line);
        }
        lines
    }
}

pub struct GameFieldPair {
    opponent: GameFieldLeft,
    player: GameFieldRight,
}

impl GameFieldPair {
 pub fn new(state: TetrisPairState, text_player: Vec<String>, text_opponent: Vec<String> ) -> Self {
        let player = GameFieldRight::new(state.player, text_player);
        let opponent = GameFieldLeft::new(state.opponent, text_opponent);
        Self { opponent, player }
 }
}

impl TermRender for GameFieldPair {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.opponent.output(style);
        let right_block = self.player.output(style);
        // Append opponent lines to player lines, padding with TermCell::Space
        for (line, mut right_line) in lines.iter_mut().zip(right_block.into_iter()) {
            line.push(TermCell::Space);
            line.push(TermCell::Space);
            line.push(TermCell::Space);
            line.push(TermCell::Space);
            line.append(&mut right_line);
        }
        lines
    }
}
