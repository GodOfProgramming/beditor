use super::BundleDnd;
use crate::{
	EditorExtension,
	inspector::{WorldExtensions as _, ui::components::ComponentInfo},
	panels::panel_dnd_drop_ui,
	private::{ ui::InspectorSelection, EditorOwned },
	ui::EditorUiBundle,
	util::{WorldExtensions as _, egui::show_docs},
};
use bevy::{ecs::world::CommandQueue, prelude::*};
use uuid::{Uuid, uuid};

#[derive(Default)]
pub struct InspectorUiExtension;

impl EditorExtension for InspectorUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<InspectorUi>();
	}

	fn build_app(&self, app: &mut App) {
		app.init_resource::<InspectorSettings>();
	}
}

#[derive(Component, Reflect, Default)]
#[require(EditorOwned)]
pub struct InspectorUi;

impl InspectorUi {
	fn dnd_ui<F>(entities: impl AsRef<[Entity]>, world: &mut World, ui: &mut egui::Ui, render_fn: F)
	where
		F: FnOnce(&mut World, &mut egui::Ui),
	{
		let (_, payload) = panel_dnd_drop_ui::<BundleDnd, ()>(ui, |ui| {
			render_fn(world, ui);
		});

		if let Some(payload) = payload {
			payload.insert(entities.as_ref().iter().cloned(), world);
		}
	}
}

impl EditorUiBundle for InspectorUi {
	type PrimaryComponent = Self;

	const NAME: &str = stringify!(Inspector);
	const ID: Uuid = uuid!("10bb68b8-c247-4792-89e9-61d1b9682a72");

	const UNIQUE: bool = true;
	const SCROLL_BARS: [bool; 2] = [true, true];

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn ui(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();

		let highlight_changes = world.resource::<InspectorSettings>().highlight_changes;

		world.resource_scope(
			|world, selection: Mut<InspectorSelection>| match selection.as_ref() {
				InspectorSelection::Entities(selected_entities) => match selected_entities.as_slice() {
					&[entity] => {
						Self::dnd_ui([entity], world, ui, |world, ui| {
							let Some(response) = world.ui_for_entity(entity, ui, highlight_changes) else {
								return;
							};

							world.queue(|_world, queue| {
								entity_context_menu(&response, queue, std::iter::once(entity));
							});

							let Some(info) = &response.body_returned else {
								return;
							};

							show_docs(&type_registry, info.type_id, response.header_response);
						});
					}
					entities => {
						Self::dnd_ui(entities, world, ui, |world, ui| {
							let Some(response) = world.ui_for_entities(ui, entities) else {
								return;
							};

							let entities = entities.to_owned();
							world.queue(|_world, queue| {
								entity_context_menu(&response, queue, entities.into_iter());
							});

							let Some(component_info) = &response.body_returned else {
								return;
							};

							show_docs(
								&type_registry,
								component_info.type_id,
								response.header_response,
							);
						});
					}
				},
				InspectorSelection::Resource(type_id, name) => {
					ui.label(name);
					world.ui_for_resource_type(ui, &type_registry, *type_id, name);
				}
				InspectorSelection::Asset(type_id, name, handle) => {
					ui.label(name);
					world.ui_for_asset(ui, &type_registry, *type_id, *handle);
				}
			},
		);
	}
}

#[derive(Resource, Default)]
pub struct InspectorSettings {
	pub highlight_changes: bool,
}

pub fn entity_context_menu(
	response: &egui::CollapsingResponse<ComponentInfo>,
	queue: &mut CommandQueue,
	entities: impl 'static + Send + Sync + Iterator<Item = Entity>,
) {
	let Some(info) = &response.body_returned else {
		return;
	};

	response.header_response.context_menu(|ui| {
		if ui.button("Remove").clicked() {
			let component_id = info.component_id;
			queue.push(move |world: &mut World| {
				for entity in entities {
					world.entity_mut(entity).remove_by_id(component_id);
				}
			});
		}
	});
}
