mod editor;
mod project;

use crate::{EditorExtension, EditorExtensionPlugin};
use bevy::prelude::*;
use editor::EditorSettingsUiExtension;
use project::ProjectSettingsUiExtension;

pub use editor::ShowEditorSettings;
pub use project::ProjectSettingsUi;

#[derive(Default)]
pub struct SettingsUiExtension;

impl EditorExtension for SettingsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		let _ = ctx;
	}

	fn build_app(&self, app: &mut App) {
		app.add_plugins((
			EditorExtensionPlugin::<EditorSettingsUiExtension>::default(),
			EditorExtensionPlugin::<ProjectSettingsUiExtension>::default(),
		));
	}
}
