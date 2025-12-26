use crate::{ui::widgets, util::components::ComponentRegistry};
use bevy::prelude::*;
use brefabs::Prefabs;
use std::{
	any::{Any, TypeId},
	num::NonZeroUsize,
	sync::Arc,
};
use vfs::{Vfs, VfsEntry, VfsNode};

pub mod assets;
pub mod components;
pub mod diagnostics;
pub mod editor_view;
pub mod hierarchy;
pub mod image_viewer;
pub mod inspector;
pub mod logs;
pub mod managed_view;
pub mod menu_bar;
pub mod prefabs;
pub mod resources;
pub mod settings;
pub mod type_editor;

pub enum BundleDnd {
	AddComponent(TypeId),
	AddPrefab(TypeId, Option<Name>),
}

impl BundleDnd {
	fn insert(&self, entities: impl Iterator<Item = Entity>, world: &mut World) -> bool {
		match self {
			BundleDnd::AddComponent(type_id) => Self::insert_component(entities, world, type_id),
			BundleDnd::AddPrefab(type_id, name) => Self::insert_prefab(entities, world, *type_id, name),
		}
	}

	fn insert_component(
		entities: impl Iterator<Item = Entity>,
		world: &mut World,
		component_id: &TypeId,
	) -> bool {
		let cr = world.resource::<ComponentRegistry>();
		let Some(component) = cr.get(component_id).cloned() else {
			warn!("Failed to lookup component");
			return false;
		};

		let component_id = component.id();

		let mut success = true;

		for entity in entities {
			if world.get_by_id(entity, component_id).is_none() {
				component.insert(entity, world);
			} else {
				success = false;
			}
		}

		success
	}

	fn insert_prefab(
		entities: impl Iterator<Item = Entity>,
		world: &mut World,
		type_id: TypeId,
		variant: &Option<Name>,
	) -> bool {
		let mut success = true;

		world.resource_scope(|world, prefabs: Mut<Prefabs>| {
			for entity in entities {
				success &= prefabs
					.apply_untyped_to(world, type_id, variant, entity)
					.is_some();
			}
		});

		success
	}
}

fn panel_dnd_drop_ui<P, R>(
	ui: &mut egui::Ui,
	render_fn: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::InnerResponse<R>, Option<Arc<P>>)
where
	P: Any + Send + Sync,
{
	// makes the whole pane droppable
	let frame = egui::Frame::default();
	let available_size = ui.available_size();

	dnd_prep_ui(ui);

	ui.dnd_drop_zone::<P, R>(frame, |ui| {
		ui.set_min_size(available_size);
		render_fn(ui)
	})
}

fn dnd_prep_ui(ui: &mut egui::Ui) {
	// fixes weird highlighting on background
	let bg_fill = ui.style().visuals.window_fill();
	ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;
}

#[derive(Default)]
pub struct SearchableVfs {
	current_node: Option<VfsNode>,
	current_path_display: String,
	filter: String,
	searched_items: Option<Vec<VfsNode>>,
}

impl SearchableVfs {
	fn current_node<T>(&mut self, vfs: &Vfs<T>) -> VfsNode {
		*self.current_node.get_or_insert_with(|| {
			let root = vfs.root();
			self.current_path_display = root.absolute(vfs).expect("root has to exist");
			root
		})
	}

	fn sync_current_node<T>(&mut self, vfs: &Vfs<T>) {
		self.current_node = self
			.current_node
			.as_ref()
			.and_then(|node| node.absolute(vfs).and_then(|ap| vfs.find_absolute(ap)));
	}

	fn set_next_node<T>(&mut self, node: VfsNode, vfs: &Vfs<T>) {
		if let Some(abs_path) = node.absolute(vfs) {
			self.current_node = Some(node);
			self.current_path_display = abs_path;
			self.searched_items.take();
			self.filter.clear();
		}
	}

	fn search_ui<T>(&mut self, ui: &mut egui::Ui, vfs: &Vfs<T>) {
		ui.vertical(|ui| {
			ui.horizontal(|ui| {
				if ui.button("Clear Search").clicked() {
					self.filter.clear();
					self.searched_items.take();
				}

				let response = ui.text_edit_singleline(&mut self.filter);

				if !self.filter.is_empty() && response.lost_focus() && !response.clicked_elsewhere() {
					self.searched_items = Some(vfs.search(&*self.filter));
				}

				response.on_hover_ui(|ui| {
					ui.label("Press Enter to perform a full search");
				});

				let current_node = self.current_node(vfs);

				if current_node.has_parent(vfs)
					&& ui
						.button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
						.clicked()
					&& let Some(parent) = current_node.parent(vfs)
				{
					self.set_next_node(parent, vfs);
				}
			});

			ui.label(&*self.current_path_display);
		});
	}

	fn display_ui<T>(
		&mut self,
		ui: &mut egui::Ui,
		vfs: &Vfs<T>,
		columns: NonZeroUsize,
		mut item_ui: impl FnMut(&mut egui::Ui, egui::Vec2, &str, egui::Id, &T),
	) {
		let mut next_path = None;

		match self.searched_items.as_ref() {
			Some(nodes) => {
				widgets::horizontal_list(ui, columns, nodes, |ui, _, node| {
					next_path = next_path.or(ui_for_entry(ui, vfs, *node, &mut item_ui));
				});
			}
			None => {
				let current_node = self.current_node(vfs);
				let nodes = vfs.ls(current_node).filter(|node| {
					self.filter.is_empty() || {
						node
							.basename(vfs)
							.map(|name| {
								name
									.to_lowercase()
									.contains(self.filter.to_lowercase().as_str())
							})
							.unwrap_or(false)
					}
				});

				widgets::horizontal_list(ui, columns, nodes, |ui, _, node| {
					next_path = next_path.or(ui_for_entry(ui, vfs, node, &mut item_ui));
				});
			}
		}

		if let Some(node) = next_path {
			self.set_next_node(node, vfs);
		}
	}
}

fn ui_for_entry<T>(
	ui: &mut egui::Ui,
	vfs: &Vfs<T>,
	node: VfsNode,
	mut item_ui: impl FnMut(&mut egui::Ui, egui::Vec2, &str, egui::Id, &T),
) -> Option<VfsNode> {
	let entry = vfs.read(node)?;
	let basename = node.basename(vfs)?;

	let card_width = ui.available_width();
	let card_height = card_width;
	let id = ui.id().with(node.node_index());
	let size = (card_width, card_height).into();

	match entry {
		VfsEntry::Dir => {
			if ui_for_dir(ui, size, basename, id) {
				return Some(node);
			}
		}
		VfsEntry::Item { value } => {
			(item_ui)(ui, size, basename, id, value);
		}
	}

	None
}

fn ui_for_dir(ui: &mut egui::Ui, size: egui::Vec2, label: &str, id: egui::Id) -> bool {
	widgets::Card::new(size)
		.with_label(label)
		.show(ui, |ui| widgets::Dir::ui(ui, id))
		.inner
		.on_hover_cursor(egui::CursorIcon::PointingHand)
		.double_clicked()
}
