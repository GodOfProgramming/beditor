mod editor;
mod project;

use bevy::prelude::*;

pub use editor::ShowEditorSettings;
pub use project::ProjectSettingsUi;

use crate::{
	EditorExtension, EditorExtensionPlugin,
	panels::settings::{editor::EditorSettingsUiExtension, project::ProjectSettingsUiExtension},
};

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
