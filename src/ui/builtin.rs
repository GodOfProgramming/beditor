use crate::util::components::ComponentRegistry;
use bevy::prelude::*;
use brefabs::Prefabs;
use std::{
	any::{Any, TypeId},
	sync::Arc,
};

pub mod assets;
pub mod components;
pub mod debug;
pub mod editor_view;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod logs;
pub mod menu_bar;
pub mod prefabs;
pub mod resources;
pub mod type_editor;

pub enum BundleDnd {
	AddComponent(TypeId),
	AddPrefab(TypeId, Option<Name>),
}

impl BundleDnd {
	fn spawn_on(&self, entities: impl Iterator<Item = Entity>, world: &mut World) -> bool {
		match self {
			BundleDnd::AddComponent(type_id) => Self::spawn_component_on(entities, world, type_id),
			BundleDnd::AddPrefab(type_id, name) => Self::spawn_prefab_on(entities, world, *type_id, name),
		}
	}

	fn spawn_component_on(
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
				component.spawn(entity, world);
			} else {
				success = false;
			}
		}

		success
	}

	fn spawn_prefab_on(
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
