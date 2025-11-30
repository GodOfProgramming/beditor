use std::any::TypeId;

use crate::{
	ui::{EditorUiBundle, InspectorSelection, builtin::dnd_drop_ui},
	util::components::ComponentRegistry,
};
use bevy::prelude::*;
use bevy_inspector_egui::bevy_inspector::{
	by_type_id::{ui_for_asset, ui_for_resource},
	ui_for_entities_shared_components, ui_for_entity,
};
use brefabs::Prefabs;
use uuid::{Uuid, uuid};

use super::InspectorDnd;

#[derive(Component, Reflect, Default)]
pub struct Inspector;

impl Inspector {
	fn inner_ui<F>(entities: impl AsRef<[Entity]>, world: &mut World, ui: &mut egui::Ui, render_fn: F)
	where
		F: FnOnce(&mut World, &mut egui::Ui),
	{
		let (_, component_id) = dnd_drop_ui(ui, |ui| {
			render_fn(world, ui);
		});

		if let Some(dnd) = component_id {
			match &*dnd {
				InspectorDnd::AddComponent(type_id) => {
					Self::spawn_component_on(type_id, entities.as_ref(), world);
				}
				InspectorDnd::AddPrefab(type_id, name) => {
					Self::spawn_prefab_on(entities.as_ref(), world, *type_id, name);
				}
			}
		}
	}

	fn spawn_component_on(component_id: &TypeId, entities: &[Entity], world: &mut World) {
		let cr = world.resource::<ComponentRegistry>();
		let Some(component) = cr.get(component_id).cloned() else {
			warn!("Failed to lookup component");
			return;
		};

		let component_id = component.id();

		for entity in entities {
			if world.get_by_id(*entity, component_id).is_none() {
				component.spawn(*entity, world);
			}
		}
	}

	fn spawn_prefab_on(
		entities: &[Entity],
		world: &mut World,
		type_id: TypeId,
		variant: &Option<Name>,
	) {
		world.resource_scope(|world, prefabs: Mut<Prefabs>| {
			for entity in entities {
				prefabs.apply_untyped_to(world, type_id, variant, *entity);
			}
		});
	}
}

impl EditorUiBundle for Inspector {
	type PrimaryComponent = Self;

	const NAME: &str = stringify!(Inspector);
	const ID: Uuid = uuid!("10bb68b8-c247-4792-89e9-61d1b9682a72");

	const UNIQUE: bool = true;

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();

		world.resource_scope(
			|world, selection: Mut<InspectorSelection>| match selection.as_ref() {
				InspectorSelection::Entities(selected_entities) => match selected_entities.as_slice() {
					&[entity] => {
						Self::inner_ui([entity], world, ui, |world, ui| {
							ui_for_entity(world, entity, ui);
						});
					}
					entities => {
						Self::inner_ui(entities, world, ui, |world, ui| {
							ui_for_entities_shared_components(world, entities, ui);
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
