use super::{EntityDnd, camera_view::CameraViewUi, image_viewer::ImageViewerUi};
use crate::{
	EditorExtension,
	inspector::{
		WorldExtensions as _,
		ui::{
			InspectorSelection, SelectEntity,
			hierarchy::{SelectedEntities, SelectedEntitiesChangedEvent},
		},
	},
	private::{
		EditorInternal, EditorInternalFilter, UserHidden,
		cam::{ActiveEditorCamera, EditorCamera, EditorManagedCamera, MoveTo, cam3d::LookAt},
		util::WorldExtensions as _,
	},
	ui::{EditorUiWorld, OpenMode, OpenUi},
};
use bevy::{
	camera::RenderTarget,
	ecs::{resource::IsResource, system::SystemIdMarker},
	gizmos_render::LineGizmoEntities,
	prelude::*,
};
use common::extensions::bevy::WorldMutExtensions as _;
use notify::Notification;
use std::sync::Arc;
use uuid::{Uuid, uuid};

#[derive(Default)]
pub struct HierarchyExtension;

impl EditorExtension for HierarchyExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<HierarchyUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.add_message::<ReparentMessage>()
			.add_message::<ClearSelectedMessage>()
			.add_observer(SelectedEntitiesChangedEvent::on_event)
			.add_systems(
				FixedUpdate,
				(
					ReparentMessage::handle,
					ClearSelectedMessage::handle,
					hide_bevy_gizmo_render_entities.run_if(resource_changed::<LineGizmoEntities>),
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

	const SCROLL_BARS: [bool; 2] = [true, true];

	fn spawn(_entity: Entity, _world: &mut World) -> Result<Self> {
		Ok(default())
	}

	fn ui(_entity: Entity, ui: &mut egui::Ui, world: &mut World) -> Result {
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

		Ok(())
	}

	fn context_menu(
		_entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		_surface: egui_dock::SurfaceIndex,
		_node: egui_dock::NodeIndex,
	) -> Result {
		if ui.button("New Empty Entity").clicked()
			&& let Some(entity) = world.spawn_stateful_entity()
		{
			world.commands().queue(SelectEntity(entity));
		}

		let editor_camera = {
			world
				.query_filtered::<Entity, EditorInternalFilter<With<EditorCamera>>>()
				.single(world)
				.ok()
		};

		if let Some(editor_camera) = editor_camera
			&& ui.button("New Empty Ui").clicked()
			&& let Some(entity) =
				world.spawn_stateful_entity_bundle((Node::default(), UiTargetCamera(editor_camera)))
		{
			world.commands().queue(SelectEntity(entity));
		}

		Ok(())
	}
}

type HierarchyFilter = (
	Without<SystemIdMarker>,
	Without<Observer>,
	Without<IsResource>,
);

impl HierarchyUi {
	fn show(ui: &mut egui::Ui, world: &mut World, selected: &mut SelectedEntities) -> bool {
		let bg_fill = ui.style().visuals.window_fill();
		ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;

		let Some(response) = (if cfg!(feature = "editor-dev") {
			world.hierarchy_ui::<(EditorInternalFilter, HierarchyFilter), EntityDnd>(
				ui,
				selected,
				dnd_handler,
			)
		} else {
			world.hierarchy_ui::<(Without<UserHidden>, HierarchyFilter), EntityDnd>(
				ui,
				selected,
				dnd_handler,
			)
		}) else {
			return false;
		};

		let Some(entity) = response.body_returned else {
			return false;
		};

		response.header_response.context_menu(|ui| {
			world.queue(|world, queue| {
				{
					let camera_state = *world.resource::<ActiveEditorCamera>();

					let mut entity_ref = world.entity_mut(entity);

					if entity_ref.contains::<Transform>() {
						ui.menu_button("Movement", |ui| {
							if camera_state == ActiveEditorCamera::Cam3D && ui.button("Look At").clicked() {
								queue.push(LookAt(entity_ref.id()));
							}

							if ui.button("Move To").clicked() {
								queue.push(MoveTo(entity_ref.id()));
							}
						});
					}

					if let Some(target) = entity_ref.get::<RenderTarget>() {
						ui.menu_button("Camera", |ui| {
							if entity_ref.contains::<EditorManagedCamera>() {
								if ui.button("Open View").clicked() {
									queue.push(OpenUi::open_with_value(
										OpenMode::Window,
										CameraViewUi::new(entity),
									));
								}
							} else if ui.button("Add To Editor").clicked() {
								queue.push(move |world: &mut World| {
									world.entity_mut(entity).insert(EditorManagedCamera);
								});
							}

							if let Some(image) = target.as_image()
								&& ui.button("Observe").clicked()
							{
								queue.push(OpenUi::open_with_value(
									OpenMode::Window,
									ImageViewerUi::new(image.id()),
								));
							}
						});
					}

					ui.menu_button("Scene", |ui| {
						if ui.button("Add Child").clicked() {
							queue.push(AddChild(entity))
						}

						if entity_ref.contains::<ChildOf>() && ui.button("Make Orphan").clicked() {
							entity_ref.remove::<ChildOf>();
						}

						if ui.button("Reparent").clicked() {
							queue.push(ReparentMessage(entity));
						}

						if ui.button("Despawn").clicked() {
							entity_ref.despawn();
						}
					});
				}

				if ui.button("Clear Selected").clicked() {
					world.write_message(ClearSelectedMessage);
				}
			});
		});

		true
	}
}

struct AddChild(Entity);

impl Command for AddChild {
	type Out = ();
	fn apply(self, world: &mut World) {
		world.spawn(ChildOf(self.0));
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

impl Command for ReparentMessage {
	type Out = ();
	fn apply(self, world: &mut World) {
		world.write_message(self);
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

fn dnd_handler(_: &mut egui::Ui, entity: Entity, world: &mut World, payload: Arc<EntityDnd>) {
	let Some(new_entity) = world.spawn_stateful_entity() else {
		return;
	};

	world.entity_mut(entity).add_child(new_entity);
	if !payload.insert(std::iter::once(new_entity), world) {
		world.trigger(Notification::error("Failed to spawn"));
	}
}

/// These entities are just names and clutter the hierarchy
fn hide_bevy_gizmo_render_entities(mut commands: Commands, entities: Res<LineGizmoEntities>) {
	for entity in [
		entities.line_gizmo_renderer,
		entities.line_strip_gizmo_renderer,
		entities.line_joint_gizmo_renderer,
	] {
		commands.entity(entity.entity()).insert(UserHidden);
	}
}
