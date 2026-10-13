use serde::{Deserialize, Serialize};

use crate::AppCommand;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyStroke {
    pub key: String,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub stroke: KeyStroke,
    pub command: AppCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keymap {
    pub version: u16,
    pub bindings: Vec<KeyBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FocusContext {
    Canvas,
    Panel,
    TextInput,
    Menu,
}

pub fn dispatch_keystroke(
    keymap: &Keymap,
    stroke: KeyStroke,
    _focus: FocusContext,
) -> Option<AppCommand> {
    keymap
        .bindings
        .iter()
        .find(|binding| binding.stroke == stroke)
        .map(|binding| binding.command.clone())
}
