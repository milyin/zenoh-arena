use crate::{
    state::TetrisPairState,
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
pub fn pad_block_right(block: &mut [Vec<TermCell>], style: &impl TermStyle) {
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
    player_name: Option<String>,
}

impl WellField {
    pub fn new(field: Field, game_over: bool) -> Self {
        Self { field, game_over, player_name: Some(String::new()) }
    }
    
    pub fn new_with_player(field: Field, game_over: bool, player_name: Option<String>) -> Self {
        Self { field, game_over, player_name }
    }
}

impl TermRender for WellField {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut lines = self.field.output(style);
        if self.player_name.is_none() {
            // Find middle line of the field
            let middle = lines.len() / 2;
            // Replace line with message
            let message = TermCell::Message("      Waiting...".to_string());
            lines[middle] = vec![message];
            // Add padding
            pad_block_right(&mut lines, style);
        } else if self.game_over {
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

pub struct GameFieldPair {
    opponent_well: WellField,
    opponent_preview: PreviewField,
    player_well: WellField,
    player_preview: PreviewField,
    message: Vec<String>,
}

impl GameFieldPair {
 pub fn new(state: TetrisPairState, message: Vec<String>) -> Self {
        let opponent_well = WellField::new_with_player(state.opponent.well, state.opponent.game_over, state.opponent.name.clone());
        let opponent_preview = PreviewField(state.opponent.preview);
        let player_well = WellField::new_with_player(state.player.well, state.player.game_over, state.player.name.clone());
        let player_preview = PreviewField(state.player.preview);
        Self { 
            opponent_well, 
            opponent_preview, 
            player_well, 
            player_preview, 
            message 
        }
 }
}

impl TermRender for GameFieldPair {
    fn output(&self, style: &impl TermStyle) -> Vec<Vec<TermCell>> {
        let mut opponent_well_lines = self.opponent_well.output(style);
        let mut opponent_preview_lines = self.opponent_preview.output(style);
        let mut player_well_lines = self.player_well.output(style);
        let mut player_preview_lines = self.player_preview.output(style);

        // Add player names after previews
        if let Some(ref name) = self.opponent_well.player_name {
            opponent_preview_lines.push(Vec::new()); // empty line
            opponent_preview_lines.push(vec![TermCell::Message(name.clone())]);
        }
        if let Some(ref name) = self.player_well.player_name {
            player_preview_lines.push(Vec::new()); // empty line
            player_preview_lines.push(vec![TermCell::Message(name.clone())]);
        }

        // Pad preview blocks to the same width
        pad_block_right(&mut opponent_preview_lines, style);
        pad_block_right(&mut player_preview_lines, style);
        pad_block_right(&mut opponent_well_lines, style);
        pad_block_right(&mut player_well_lines, style);

        // Calculate widths for each section (they're already padded internally)
        let opponent_well_width = if opponent_well_lines.is_empty() {
            0
        } else {
            opponent_well_lines[0].iter().map(|c| style.width(c)).sum()
        };
        let opponent_preview_width = if opponent_preview_lines.is_empty() {
            0
        } else {
            opponent_preview_lines[0].iter().map(|c| style.width(c)).sum()
        };
        let player_preview_width = if player_preview_lines.is_empty() {
            0
        } else {
            player_preview_lines[0].iter().map(|c| style.width(c)).sum()
        };
        let player_well_width = if player_well_lines.is_empty() {
            0
        } else {
            player_well_lines[0].iter().map(|c| style.width(c)).sum()
        };

        // Calculate max message width
        let max_message_width = self.message.iter()
            .map(|m| m.len())
            .max()
            .unwrap_or(0);

        let mut lines = Vec::new();
        let preview_len = opponent_preview_lines.len().max(player_preview_lines.len());
        let well_len = opponent_well_lines.len().max(player_well_lines.len());
        let total_lines = well_len;
        
        // Middle section width: opponent_preview + 2 spaces + player_preview
        let middle_section_width = opponent_preview_width + 2 + player_preview_width;
        // Ensure message width is at least as wide as the middle section
        let message_section_width = middle_section_width.max(max_message_width);

        // Generate all lines
        for i in 0..total_lines {
            let mut line = Vec::new();
            
            // Opponent well
            if i < opponent_well_lines.len() {
                line.append(&mut opponent_well_lines[i].clone());
            } else {
                // Pad with spaces if this line doesn't exist
                for _ in 0..opponent_well_width {
                    line.push(TermCell::Space);
                }
            }
            line.push(TermCell::Space);
            
            // Middle section: either previews or message
            if i < preview_len {
                // Show previews: <opponent_preview> <player_preview>
                
                // Opponent preview
                if i < opponent_preview_lines.len() {
                    line.append(&mut opponent_preview_lines[i].clone());
                } else {
                    // Pad with spaces if this line doesn't exist
                    for _ in 0..opponent_preview_width {
                        line.push(TermCell::Space);
                    }
                }
                line.push(TermCell::Space);
                line.push(TermCell::Space);
                
                // Player preview
                if i < player_preview_lines.len() {
                    line.append(&mut player_preview_lines[i].clone());
                } else {
                    // Pad with spaces if this line doesn't exist
                    for _ in 0..player_preview_width {
                        line.push(TermCell::Space);
                    }
                }
                
                // Pad middle section to full width if needed
                let current_middle_width = opponent_preview_width + 2 + player_preview_width;
                for _ in current_middle_width..message_section_width {
                    line.push(TermCell::Space);
                }
            } else {
                // Show message
                line.push(TermCell::Space);
                
                let message_idx = i - preview_len;
                if message_idx < self.message.len() {
                    let msg = &self.message[message_idx];
                    line.push(TermCell::Message(msg.clone()));
                    let padding = message_section_width - 2 - msg.len();
                    for _ in 0..padding {
                        line.push(TermCell::Space);
                    }
                } else {
                    // Pad with spaces
                    for _ in 0..(message_section_width - 2) {
                        line.push(TermCell::Space);
                    }
                }
                line.push(TermCell::Space);
            }
            
            line.push(TermCell::Space);
            
            // Player well
            if i < player_well_lines.len() {
                line.append(&mut player_well_lines[i].clone());
            } else {
                // Pad with spaces if this line doesn't exist
                for _ in 0..player_well_width {
                    line.push(TermCell::Space);
                }
            }
            
            lines.push(line);
        }

        lines
    }
}
