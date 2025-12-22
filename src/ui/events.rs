use super::UiManager;
use crate::{EditorUiBundle, ui::TabState};
use bevy::prelude::*;
use derive_new::new;
use egui_dock::{NodeIndex, SurfaceIndex};
use smallvec::SmallVec;
use std::marker::PhantomData;
use transform_gizmo_bevy::GizmoTarget;

#[derive(new, Message)]
pub struct OpenUiMessage(Vec<TabState>);

impl OpenUiMessage {
	pub fn handle(mut messages: MessageReader<Self>, mut ui_manager: ResMut<UiManager>) {
		for msg in messages.read() {
			ui_manager.add_detached(msg.0.clone());
		}
	}
}

#[derive(Message)]
pub struct OpenSingleUiMessage {
	cmd: Box<dyn Send + Sync + Fn(&mut Commands)>,
}

impl OpenSingleUiMessage {
	pub fn new<T>() -> Self
	where
		T: EditorUiBundle,
	{
		Self {
			cmd: Box::new(|commands| commands.queue(OpenSingleUi::<T>::new())),
		}
	}

	pub fn handle(mut messages: MessageReader<Self>, mut commands: Commands) {
		for msg in messages.read() {
			(msg.cmd)(&mut commands);
		}
	}
}

pub struct OpenSingleUi<T>
where
	T: EditorUiBundle,
{
	_pd: PhantomData<T>,
}

impl<T> OpenSingleUi<T>
where
	T: EditorUiBundle,
{
	pub fn new() -> Self {
		Self { _pd: default() }
	}
}

impl<T> Command for OpenSingleUi<T>
where
	T: EditorUiBundle,
{
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
			let tab = TabState::spawn::<T>(world);
			ui_manager.add_detached(vec![tab]);
		});
	}
}

#[derive(new, Message, Clone)]
pub struct AppendUiMessage(SurfaceIndex, NodeIndex, TabState);

impl AppendUiMessage {
	pub fn handle(mut messages: MessageReader<Self>, mut ui_manager: ResMut<UiManager>) {
		for msg in messages.read() {
			let AppendUiMessage(surface, node, tab) = msg;
			ui_manager.add_tab(*surface, *node, tab.clone());
		}
	}
}

#[derive(EntityEvent, new, Clone, Copy)]
pub struct RemoveUiEvent(Entity);

impl RemoveUiEvent {
	pub fn on_event(event: On<Self>, mut commands: Commands) {
		let RemoveUiEvent(tab) = *event;
		commands.entity(tab).despawn();
	}
}

#[derive(new, Event)]
pub struct SyncGizmoTargetsEvent {
	current: SmallVec<[Entity; 8]>,
	removed: SmallVec<[Entity; 8]>,
}

impl SyncGizmoTargetsEvent {
	pub fn on_event(event: On<Self>, mut commands: Commands) {
		for entity in event.current.iter() {
			commands.entity(*entity).insert(GizmoTarget::default());
		}

		for entity in event.removed.iter() {
			commands.entity(*entity).remove::<GizmoTarget>();
		}
	}
}
