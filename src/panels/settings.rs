mod editor;
mod project;

use bevy::prelude::*;
use convert_case::{Case, Casing};

pub use editor::EditorSettingsUi;
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

fn settings_display<C>(
	ui: &mut egui::Ui,
	selected_category: Option<C>,
	category_list: impl Iterator<Item = C>,
	content: impl FnOnce(&mut egui::Ui),
) -> Option<C>
where
	C: std::fmt::Display + Eq + Copy,
{
	let mut out = None;

	widgets::DualVScrollArea::new(ui.id(), ui.available_width() * 0.1).show(
		ui,
		|ui| {
			ui.heading("Categories");
			ui.separator();

			for item in category_list {
				if ui
					.selectable_label(
						selected_category == Some(item),
						item.to_string().to_case(Case::Title),
					)
					.clicked()
				{
					out = Some(item);
				}
			}
		},
		|ui| {
			if let Some(selected_category) = selected_category {
				ui.heading(selected_category.to_string().to_case(Case::Title));
				ui.separator();
			}

			(content)(ui);
		},
	);

	out
}
