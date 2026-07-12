use super::UiManager;
use crate::private::ui::TabState;
use bevy::prelude::*;
use derive_new::new;
use egui_dock::{NodeIndex, SurfaceIndex};

#[derive(new, Message, Clone)]
pub struct AppendUiMessage(SurfaceIndex, NodeIndex, TabState);

impl AppendUiMessage {
	pub fn handle(mut messages: MessageReader<Self>, mut ui_manager: ResMut<UiManager>) {
		for msg in messages.read() {
			let Self(surface, node, tab) = msg;
			ui_manager.append_tab(*surface, *node, *tab);
		}
	}
}

#[derive(EntityEvent, new, Clone, Copy)]
pub struct RemoveUiEvent(Entity);

impl RemoveUiEvent {
	pub fn on_event(event: On<Self>, mut commands: Commands) {
		commands.entity(event.event_target()).despawn();
	}
}
