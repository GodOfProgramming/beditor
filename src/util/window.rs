use bevy::{
	prelude::*,
	window::{CursorGrabMode, CursorIcon, CursorOptions},
};

pub fn show_cursor(cursor: &mut CursorOptions) {
	cursor.visible = true;
	cursor.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(cursor: &mut CursorOptions) {
	cursor.visible = false;
	cursor.grab_mode = CursorGrabMode::Locked;
}

pub fn set_cursor_icon(commands: &mut Commands, entity: Entity, cursor: impl Into<CursorIcon>) {
	commands.entity(entity).insert(cursor.into());
}
