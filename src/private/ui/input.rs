use bevy::{
	ecs::system::entity_command,
	input_focus::InputFocus,
	picking::{
		hover::{PickingInteraction, update_interactions},
		pointer::PointerId,
	},
	prelude::*,
	ui::ui_focus_system,
};
use bevy_egui::EguiContext;
use itertools::Itertools;

use crate::{
	inspector::ui::{InspectorSelection, Selected},
	private::{
		EditorInternalQuery, EditorInternalSingle,
		ext::{camera_view::CameraViewPointers, scene_view::SceneViewUi},
		util::entity::insert_bundle_from_world,
	},
};

pub struct EditorUiInputPlugin;

impl Plugin for EditorUiInputPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_state::<KeyboardFocus>()
			.add_observer(handle_click_events)
			.add_observer(handle_selected)
			.add_observer(handle_deselected)
			.add_systems(
				PreUpdate,
				forward_interactions
					.after(ui_focus_system)
					.after(update_interactions),
			)
			.add_systems(FixedUpdate, auto_register_sprites)
			.add_systems(
				super::EditorUiEguiContextPass,
				KeyboardFocus::set_state.after(super::ui),
			);
	}
}

fn handle_click_events(
	mut event: On<Pointer<Click>>,
	mut commands: Commands,
	editor_pointers: Option<EditorInternalSingle<&CameraViewPointers, With<SceneViewUi>>>,
	q_pointer_ids: EditorInternalQuery<&PointerId>,
	mut selection: ResMut<InspectorSelection>,
	keyboard: Res<ButtonInput<KeyCode>>,
) {
	let Some(editor_pointers) = editor_pointers else {
		return;
	};

	let mut pointer_ids = editor_pointers
		.iter()
		.filter_map(|e| q_pointer_ids.get(e).ok());

	match event.button {
		PointerButton::Primary => {
			if !pointer_ids.contains(&event.pointer_id) {
				return;
			}

			event.propagate(false);

			let target = event.event_target();

			let maybe_add =
				keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

			let event = selection.add_selected(target, maybe_add);
			commands.trigger(event);
		}
		PointerButton::Secondary => (),
		PointerButton::Middle => (),
	}
}

fn handle_selected(
	event: On<Add, Selected>,
	mut commands: Commands,
	q_3d_meshes: Query<(), With<Mesh3d>>,
	q_transforms: Query<(), With<Transform>>,
) {
	let entity = event.event_target();
	if q_transforms.contains(entity)
		&& let Ok(mut entity_commands) = commands.get_entity(entity)
	{
		entity_commands.insert(transform_gizmo_bevy::GizmoTarget::default());

		if q_3d_meshes.contains(entity_commands.id()) {
			entity_commands.queue_handled(
				insert_bundle_from_world::<super::Highlight>(),
				|err, ctx| {
					error!(ctx = ctx.to_string(), "{err}");
				},
			);
		}
	}
}

fn handle_deselected(event: On<Remove, Selected>, mut commands: Commands) {
	if let Ok(mut entity) = commands.get_entity(event.event_target()) {
		entity.queue_silenced(entity_command::remove::<(
			transform_gizmo_bevy::GizmoTarget,
			super::Highlight,
		)>());
	}
}

/// This exists as a state because you need to have immutable data in a run_if
/// and egui contexts need mutable access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum KeyboardFocus {
	#[default]
	Unfocused,
	Focused,
}

impl KeyboardFocus {
	fn set_state(
		mut q_contexts: EditorInternalQuery<&mut EguiContext>,
		mut keyboard_focus: ResMut<NextState<Self>>,
		input_focus: Res<InputFocus>,
	) {
		let egui_has_focus = q_contexts
			.iter_mut()
			.any(|mut ctx| ctx.get_mut().egui_wants_keyboard_input());

		let bevy_has_focus = input_focus.get().is_some();

		if egui_has_focus || bevy_has_focus {
			keyboard_focus.set(KeyboardFocus::Focused);
		} else {
			keyboard_focus.set(KeyboardFocus::Unfocused);
		}
	}
}

fn forward_interactions(
	mut q_interactions: EditorInternalQuery<(&mut Interaction, &PickingInteraction)>,
) {
	for (mut entity_interaction, picking_interaction) in &mut q_interactions {
		let interaction = match picking_interaction {
			PickingInteraction::Pressed => Interaction::Pressed,
			PickingInteraction::Hovered => Interaction::Hovered,
			PickingInteraction::None => Interaction::None,
		};

		if *entity_interaction != interaction {
			*entity_interaction = interaction;
		}
	}
}

fn auto_register_sprites(
	mut commands: Commands,
	q_sprites: Query<Entity, (With<Sprite>, Without<Pickable>)>,
) {
	for entity in q_sprites {
		commands.entity(entity).insert(Pickable::default());
	}
}
