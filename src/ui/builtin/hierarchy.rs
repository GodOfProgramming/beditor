use std::sync::Arc;

use crate::{
	inspector::WorldExtensions,
	ui::{
		EditorUiBundle, InspectorSelection, SelectedEntities, builtin::BundleDnd,
		notifications::Notification,
	},
	util::reflection,
};
use bevy::prelude::*;
use egui_file_dialog::FileDialog;
use uuid::{Uuid, uuid};

#[derive(Component, Reflect, Default)]
pub struct Hierarchy;

impl EditorUiBundle for Hierarchy {
	type PrimaryComponent = Self;

	const NAME: &str = stringify!(Hierarchy);
	const ID: Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

	const UNIQUE: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, false];

	fn init(app: &mut App) {
		app
			.init_resource::<HierarchyState>()
			.add_message::<ReparentMessage>()
			.add_message::<ClearSelectedMessage>()
			.add_message::<DespawnEntityMessage>()
			.add_systems(bevy_egui::EguiPrimaryContextPass, show_dialogs)
			.add_systems(
				FixedUpdate,
				(
					ReparentMessage::handle,
					ClearSelectedMessage::handle,
					DespawnEntityMessage::handle,
				),
			);
	}

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn ui(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		world.resource_scope(|world, mut selection: Mut<InspectorSelection>| {
			if let InspectorSelection::Entities(selected_entities) = selection.as_mut() {
				Self::show(ui, world, selected_entities);
			} else {
				let mut selected_entities = SelectedEntities::default();
				if Self::show(ui, world, &mut selected_entities) {
					*selection = InspectorSelection::Entities(selected_entities);
				}
			}
		});
	}
}

impl Hierarchy {
	fn show(ui: &mut egui::Ui, world: &mut World, selected: &mut SelectedEntities) -> bool {
		let bg_fill = ui.style().visuals.window_fill();
		ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;

		let Some(response) = world.hierarchy_ui::<(), BundleDnd>(ui, selected, dnd_handler) else {
			return false;
		};

		let Some(entity) = response.body_returned else {
			return false;
		};

		response.header_response.context_menu(|ui| {
			if ui.button("Reparent Selected").clicked() {
				world.write_message(ReparentMessage(entity));
			}

			let mut entity_ref = world.entity_mut(entity);
			if entity_ref.get::<ChildOf>().is_some() && ui.button("Remove Parent").clicked() {
				entity_ref.remove::<ChildOf>();
			}

			let state = world
				.resource::<HierarchyState>()
				.file_dialog
				.state()
				.clone();
			ui.add_enabled_ui(state != egui_file_dialog::DialogState::Open, |ui| {
				if ui.button("Save As Scene").clicked() {
					match reflection::scenes::serialize_to_scene(entity, world) {
						Ok(data) => {
							let mut state = world.resource_mut::<HierarchyState>();
							state.file_dialog.save_file();
							state.data = data;
						}
						Err(err) => {
							world.trigger(Notification::error("Failed to save scene").with_context(err))
						}
					}
				}
			});

			if ui.button("Despawn").clicked() {
				world.write_message(DespawnEntityMessage(entity));
			}

			if ui.button("Clear Selected").clicked() {
				world.write_message(ClearSelectedMessage);
			}
		});

		true
	}
}

#[derive(Message)]
struct ReparentMessage(Entity);

impl ReparentMessage {
	fn handle(
		mut message_reader: MessageReader<Self>,
		mut commands: Commands,
		mut selection: ResMut<InspectorSelection>,
	) {
		for msg in message_reader.read() {
			if let InspectorSelection::Entities(selected) = &*selection {
				commands.entity(msg.0).add_children(selected.as_slice());

				let mut entities = SelectedEntities::default();
				let event = entities.select_replace(msg.0);
				commands.trigger(event);

				*selection = InspectorSelection::Entities(entities)
			}
		}
	}
}

#[derive(Message)]
struct DespawnEntityMessage(Entity);

impl DespawnEntityMessage {
	fn handle(mut messages: MessageReader<Self>, mut commands: Commands) {
		for msg in messages.read() {
			commands.entity(msg.0).despawn();
		}
	}
}

#[derive(Message)]
struct ClearSelectedMessage;

impl ClearSelectedMessage {
	fn handle(
		mut messages: MessageReader<Self>,
		mut commands: Commands,
		mut inspector_selection: ResMut<InspectorSelection>,
	) {
		if messages.is_empty() {
			return;
		}

		messages.clear();

		if let InspectorSelection::Entities(selected) = inspector_selection.as_mut() {
			let event = selected.scoped_clear();
			commands.trigger(event);
		}
	}
}

#[derive(Resource, Default)]
struct HierarchyState {
	file_dialog: FileDialog,
	data: Vec<u8>,
}

fn show_dialogs(
	mut commands: Commands,
	mut state: ResMut<HierarchyState>,
	mut contexts: bevy_egui::EguiContexts,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		commands.trigger(Notification::error("Failed to get egui context"));
		return;
	};

	state.file_dialog.update(ctx);
	if let Some(file) = state.file_dialog.take_picked()
		&& state.file_dialog.mode() == egui_file_dialog::DialogMode::SaveFile
	{
		match std::fs::write(&file, std::mem::take(&mut state.data)) {
			Ok(_) => {
				commands.trigger(Notification::success(format!(
					"Saved scene to {}",
					file.display()
				)));
			}
			Err(err) => {
				commands.trigger(
					Notification::error(format!("Failed to save scene to {}", file.display()))
						.with_context(err),
				);
			}
		}
	}
}

fn dnd_handler(_: &mut egui::Ui, entity: Entity, world: &mut World, payload: Arc<BundleDnd>) {
	let new_entity = world.spawn_empty().id();
	world.entity_mut(entity).add_child(new_entity);
	if !payload.spawn_on(std::iter::once(new_entity), world) {
		world.trigger(Notification::error("Failed to spawn"));
	}
}
