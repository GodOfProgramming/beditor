use super::managers::UiManager;
use bevy::prelude::*;
use derive_new::new;
use egui_dock::{NodeIndex, SurfaceIndex};

#[derive(Message, new, Clone, Copy)]
pub struct AddUiMessage(SurfaceIndex, NodeIndex, Entity);

impl AddUiMessage {
  pub fn handle(mut messages: MessageReader<Self>, mut ui_manager: ResMut<UiManager>) {
    for msg in messages.read() {
      let AddUiMessage(surface, node, tab) = *msg;
      ui_manager.add_tab(surface, node, tab);
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
