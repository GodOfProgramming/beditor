use std::sync::Arc;

use crate::{
	EditorExtension,
	inspector::{
		WorldExtensions as _,
		ui::hierarchy::{SelectedEntities, SelectedEntitiesChangedEvent},
	},
	panels::{BundleDnd, image_viewer::OpenImageViewer},
	private::{
		EditorInternal, EditorInternalFilter, EditorInternalSingle, UserHidden,
		cam::{
			ActiveEditorCamera, EditorManagedCamera,
			commands::{LookAt, MoveTo},
		},
		scene,
		ui::{EditorEguiContext, EditorUiEguiContextPass, InspectorSelection},
	},
	ui::EditorUiWorld,
	util::WorldExtensions as _,
};
use bevy::prelude::*;
use bevy_egui::EguiContext;
use egui_file_dialog::FileDialog;
use notify::Notification;
use uuid::{Uuid, uuid};

#[derive(Default)]
pub struct HierarchyExtension;

impl EditorExtension for HierarchyExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<HierarchyUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.init_resource::<HierarchyState>()
			.add_message::<ReparentMessage>()
			.add_message::<ClearSelectedMessage>()
			.add_message::<DespawnEntityMessage>()
			.add_observer(SelectedEntitiesChangedEvent::on_event)
			.add_systems(EditorUiEguiContextPass, show_dialogs)
			.add_systems(
				FixedUpdate,
				(
					ReparentMessage::handle,
					ClearSelectedMessage::handle,
					DespawnEntityMessage::handle,
				),
			);
	}
}

#[derive(Component, Reflect, Default)]
#[require(EditorInternal)]
pub struct HierarchyUi;

impl EditorUiWorld for HierarchyUi {
	type MarkerComponent = Self;

	const NAME: &str = stringify!(Hierarchy);
	const ID: Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

	const UNIQUE: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, false];

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

	fn context_menu(
		_entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		_surface: egui_dock::SurfaceIndex,
		_node: egui_dock::NodeIndex,
	) {
		if ui.button("Spawn New Entity").clicked()
			&& let Some(entity) = world.spawn_stateful_entity()
		{
			let mut inspector_selection = world.resource_mut::<InspectorSelection>();
			let event = inspector_selection.add_selected(entity, false);
			world.trigger(event);
		}
	}
}

impl HierarchyUi {
	fn show(ui: &mut egui::Ui, world: &mut World, selected: &mut SelectedEntities) -> bool {
		let bg_fill = ui.style().visuals.window_fill();
		ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;

		let Some(response) = (if cfg!(feature = "editor-dev") {
			world.hierarchy_ui::<EditorInternalFilter, BundleDnd>(ui, selected, dnd_handler)
		} else {
			world.hierarchy_ui::<Without<UserHidden>, BundleDnd>(ui, selected, dnd_handler)
		}) else {
			return false;
		};

		let Some(entity) = response.body_returned else {
			return false;
		};

		response.header_response.context_menu(|ui| {
			world.queue(|world, queue| {
				{
					let camera_state = world.state::<ActiveEditorCamera>();

					let mut entity_ref = world.entity_mut(entity);

					if camera_state.is_active() && entity_ref.contains::<Transform>() {
						if camera_state.is_3d() && ui.button("Look At").clicked() {
							queue.push(LookAt(entity_ref.id()));
						}

						if ui.button("Move To").clicked() {
							queue.push(MoveTo(entity_ref.id()));
						}
					}

					if entity_ref.get::<ChildOf>().is_some() && ui.button("Make Orphan").clicked() {
						entity_ref.remove::<ChildOf>();
					}

					if let Some(camera) = entity_ref.get::<Camera>()
						&& entity_ref.get::<EditorManagedCamera>().is_none()
						&& let Some(image) = camera.target.as_image()
						&& ui.button("View").clicked()
					{
						queue.push(OpenImageViewer(image.clone()));
					}
				}

				if ui.button("Reparent").clicked() {
					world.write_message(ReparentMessage(entity));
				}

				let state = world
					.resource::<HierarchyState>()
					.scene_file_dialog
					.state()
					.clone();

				ui.add_enabled_ui(state != egui_file_dialog::DialogState::Open, |ui| {
					if ui.button("Save As Scene").clicked() {
						match scene::serialize_to_scene(entity, world) {
							Ok(data) => {
								let mut state = world.resource_mut::<HierarchyState>();
								state.scene_file_dialog.save_file();
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
	scene_file_dialog: FileDialog,
	data: Vec<u8>,
}

fn show_dialogs(
	mut commands: Commands,
	mut state: ResMut<HierarchyState>,
	mut context: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
) {
	let ctx = context.get_mut();

	state.scene_file_dialog.update(ctx);
	if let Some(file) = state.scene_file_dialog.take_picked()
		&& state.scene_file_dialog.mode() == egui_file_dialog::DialogMode::SaveFile
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
	let Some(new_entity) = world.spawn_stateful_entity() else {
		return;
	};

	world.entity_mut(entity).add_child(new_entity);
	if !payload.insert(std::iter::once(new_entity), world) {
		world.trigger(Notification::error("Failed to spawn"));
	}
}
