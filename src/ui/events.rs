use crate::ui::TabState;

use super::UiManager;
use bevy::prelude::*;
use derive_new::new;
use egui_dock::{NodeIndex, SurfaceIndex};

#[derive(Message, new, Clone)]
pub struct AddUiMessage(SurfaceIndex, NodeIndex, TabState);

impl AddUiMessage {
	pub fn handle(mut messages: MessageReader<Self>, mut ui_manager: ResMut<UiManager>) {
		for msg in messages.read() {
			let AddUiMessage(surface, node, tab) = msg;
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
