mod editor;
mod project;

use bevy::prelude::*;
use convert_case::{Case, Casing};

pub use editor::EditorSettingsUi;
pub use project::ProjectSettingsUi;

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

	ui.allocate_ui_with_layout(
		ui.available_size(),
		egui::Layout::left_to_right(egui::Align::Center),
		|ui| {
			egui::ScrollArea::vertical()
				.max_width(ui.available_width() * 0.1)
				.auto_shrink([false; 2])
				.id_salt("categories")
				.show(ui, |ui| {
					ui.vertical(|ui| {
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
					});
				});

			ui.separator();

			egui::ScrollArea::vertical()
				.auto_shrink([true, false])
				.id_salt("contents")
				.show(ui, |ui| {
					ui.vertical(|ui| {
						if let Some(selected_category) = selected_category {
							ui.heading(selected_category.to_string().to_case(Case::Title));
							ui.separator();
						}

						(content)(ui);
					});
				});
		},
	);

	out
}
