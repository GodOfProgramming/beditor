use super::BundleDnd;
use crate::{
	EditorUi,
	ui::widgets::{Card, horizontal_list},
	util::components::{ComponentRegistry, RegisteredComponent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::marker::PhantomData;
use uuid::uuid;
use vfs::{VfsEntry, VfsNode};

#[derive(Component, Reflect)]
pub struct Components {
	components_per_row: usize,
}

impl Default for Components {
	fn default() -> Self {
		Self {
			components_per_row: 20,
		}
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	component_registry: Res<'w, ComponentRegistry>,

	current_node: Local<'s, Option<VfsNode>>,
	current_path_display: Local<'s, String>,

	filter: Local<'s, String>,

	_pd: PhantomData<&'s ()>,
}

impl EditorUi for Components {
	const NAME: &str = "Components";

	const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

	const UNIQUE: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, true];

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			component_registry,
			mut current_node,
			mut filter,
			current_path_display: mut current_node_display,
			_pd: _,
		} = params;

		let vfs = component_registry.vfs();

		let current_node = current_node.get_or_insert_with(|| {
			let root = vfs.root();
			*current_node_display = root.absolute(vfs).expect("root has to exist");
			root
		});

		ui.horizontal(|ui| {
			ui.text_edit_singleline(&mut *filter);

			if current_node.has_parent(vfs)
				&& ui
					.button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
					.clicked()
				&& let Some(parent) = current_node.parent(vfs)
			{
				*current_node = parent;
			}
		});

		ui.label(&*current_node_display);

		let components = vfs.ls(*current_node).filter(|node| {
			filter.is_empty() || {
				node
					.basename(vfs)
					.map(|name| name.to_lowercase().contains(filter.to_lowercase().as_str()))
					.unwrap_or(false)
			}
		});

		let mut next_path = None;
		let num_columns = self.components_per_row.max(1);

		horizontal_list(ui, num_columns, components, |ui, i, node| {
			let card_width = ui.available_width();
			let card_height = card_width;

			let Some(entry) = vfs.read(node) else {
				return;
			};

			let Some(basename) = node.basename(vfs) else {
				return;
			};

			match entry {
				VfsEntry::Dir => {
					if ui_for_dir(ui, (card_width, card_height), basename, i) {
						next_path = Some(node);
					}
				}
				VfsEntry::Item { value } => {
					if let Some(component) = component_registry.get(value) {
						ui_for_item(ui, (card_width, card_height), basename, component);
					}
				}
			}
		});

		if let Some(node) = next_path
			&& let Some(abs_path) = node.absolute(vfs)
		{
			*current_node = node;
			*current_node_display = abs_path;
		}
	}
}

fn ui_for_dir(ui: &mut egui::Ui, size: impl Into<egui::Vec2>, label: &str, i: usize) -> bool {
	let size = size.into();
	Card::new(size)
		.with_label(label)
		.show(ui, |ui| {
			ui.label(egui_phosphor_icons::icons::FOLDER.regular());

			ui.interact(ui.min_rect(), ui.id().with(i), egui::Sense::click())
		})
		.inner
		.on_hover_cursor(egui::CursorIcon::PointingHand)
		.double_clicked()
}

fn ui_for_item(
	ui: &mut egui::Ui,
	size: impl Into<egui::Vec2>,
	label: &str,
	component: &RegisteredComponent,
) {
	let size = size.into();
	let id = component.type_id();
	ui.dnd_drag_source(egui::Id::new(id), BundleDnd::AddComponent(id), |ui| {
		Card::new(size).with_label(label).show(ui, |ui| {
			ui.label(egui_phosphor_icons::icons::PUZZLE_PIECE.regular());
		});
	});
}
