use super::EditorCamera;
use bevy::prelude::*;
use derive_new::new;
use notify::Notification;

#[derive(new, EntityEvent)]
pub struct MoveTo(pub Entity);

impl MoveTo {
	pub(super) fn handle(
		event: On<Self>,
		mut commands: Commands,
		mut q_transforms: Query<&mut Transform>,
		q_cams: Query<Entity, With<EditorCamera>>,
	) {
		let entity = event.event_target();
		let Ok(target) = q_transforms.get(entity).cloned() else {
			commands.trigger(
				Notification::warn("Tried to look at entity with no transform").with_context(
					serde_json::json!({
						"entity": entity
					}),
				),
			);
			return;
		};

		for cam in &q_cams {
			if let Ok(mut transform) = q_transforms.get_mut(cam) {
				transform.translation = target.translation;
			}
		}
	}
}

impl Command for MoveTo {
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}

#[derive(new, EntityEvent)]
pub struct LookAt(pub Entity);

impl LookAt {
	pub(super) fn handle(
		event: On<Self>,
		mut commands: Commands,
		mut q_transforms: Query<&mut Transform>,
		q_cams: Query<Entity, With<EditorCamera>>,
	) {
		let entity = event.event_target();
		let Ok(target) = q_transforms.get(entity).cloned() else {
			commands.trigger(
				Notification::warn("Tried to look at entity with no transform").with_context(
					serde_json::json!({
						"entity": entity
					}),
				),
			);
			return;
		};

		for cam in &q_cams {
			if let Ok(mut transform) = q_transforms.get_mut(cam) {
				transform.look_at(target.translation, Vec3::Y);
			}
		}
	}
}

impl Command for LookAt {
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}
