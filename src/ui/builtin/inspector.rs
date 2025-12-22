use std::any::TypeId;

use super::BundleDnd;
use crate::ui::{EditorUiBundle, InspectorSelection, builtin::panel_dnd_drop_ui};
use bevy::{ecs::component::ComponentId, prelude::*, reflect::TypeRegistry};
use bevy_inspector_egui::bevy_inspector::by_type_id::{ui_for_asset, ui_for_resource};
use uuid::{Uuid, uuid};

#[derive(Component, Reflect, Default)]
pub struct Inspector;

impl Inspector {
	fn dnd_ui<F>(entities: impl AsRef<[Entity]>, world: &mut World, ui: &mut egui::Ui, render_fn: F)
	where
		F: FnOnce(&mut World, &mut egui::Ui),
	{
		let (_, component_id) = panel_dnd_drop_ui::<BundleDnd, ()>(ui, |ui| {
			render_fn(world, ui);
		});

		if let Some(dnd) = component_id {
			dnd.spawn_on(entities.as_ref().iter().cloned(), world);
		}
	}
}

impl EditorUiBundle for Inspector {
	type PrimaryComponent = Self;

	const NAME: &str = stringify!(Inspector);
	const ID: Uuid = uuid!("10bb68b8-c247-4792-89e9-61d1b9682a72");

	const UNIQUE: bool = true;
	const SCROLL_BARS: [bool; 2] = [true, true];

	fn init(app: &mut App) {
		app.init_resource::<InspectorSettings>();
	}

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();

		let highlight_changes = world.resource::<InspectorSettings>().highlight_changes;

		world.resource_scope(
			|world, selection: Mut<InspectorSelection>| match selection.as_ref() {
				InspectorSelection::Entities(selected_entities) => match selected_entities.as_slice() {
					&[entity] => {
						Self::dnd_ui([entity], world, ui, |world, ui| {
							bevy_inspector_egui::bevy_inspector::mods::ui_for_entity(
								world,
								entity,
								ui,
								Some(|ui, entity, world, type_registry, component_id, type_id| {
									context_menu(
										ui,
										std::iter::once(entity),
										world,
										type_registry,
										component_id,
										type_id,
									);
								}),
								highlight_changes,
							);
						});
					}
					entities => {
						Self::dnd_ui(entities, world, ui, |world, ui| {
							bevy_inspector_egui::bevy_inspector::mods::ui_for_entities_shared_components(
								world,
								entities,
								ui,
								Some(
									|ui, entities, world, type_registry, component_id, type_id| {
										context_menu(
											ui,
											entities.iter().cloned(),
											world,
											type_registry,
											component_id,
											type_id,
										);
									},
								),
							);
						});
					}
				},
				InspectorSelection::Resource(type_id, name) => {
					ui.label(name);
					ui_for_resource(world, *type_id, ui, name, &type_registry)
				}
				InspectorSelection::Asset(type_id, name, handle) => {
					ui.label(name);
					ui_for_asset(world, *type_id, *handle, ui, &type_registry);
				}
			},
		);
	}
}

#[derive(Resource, Default)]
pub struct InspectorSettings {
	pub highlight_changes: bool,
}

fn context_menu(
	ui: &mut egui::Ui,
	entities: impl Iterator<Item = Entity>,
	world: &mut World,
	_type_registry: &TypeRegistry,
	component_id: ComponentId,
	_type_id: TypeId,
) {
	if ui.button("Remove").clicked() {
		for entity in entities {
			world.entity_mut(entity).remove_by_id(component_id);
		}
	}
}
