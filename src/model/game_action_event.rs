use super::Difficulty;

#[derive(Debug, Clone)]
pub enum GameActionEvent {
    CellClick(usize, usize, Option<char>),
    CellRightClick(usize, usize, Option<char>),
    HorizontalClueClick(usize),       // clue_idx
    VerticalClueClick(usize),         // clue_idx
    NewGame(Difficulty, Option<u64>), // grid rows, grid columns
    InitDisplay,
    CompletePuzzle,
    Solve,
    RewindLastGood,
    IncrementHintsUsed,
    ShowHint,
    Undo,
    Redo,
    Pause,
    Resume,
    Quit,
    Submit,
    Restart,
}
