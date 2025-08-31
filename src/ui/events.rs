use super::managers::UiManager;
use bevy::prelude::*;
use derive_new::new;
use egui_dock::{NodeIndex, SurfaceIndex};

#[derive(Event, new, Clone, Copy)]
pub struct AddUiEvent(SurfaceIndex, NodeIndex, Entity);

impl AddUiEvent {
  pub fn on_event(mut events: EventReader<Self>, mut ui_manager: ResMut<UiManager>) {
    for event in events.read() {
      let AddUiEvent(surface, node, tab) = *event;

      let Some(surface) = ui_manager.surface_mut(surface) else {
        continue;
      };

      let Some(nodes) = surface.node_tree_mut() else {
        continue;
      };

      let node = &mut nodes[node];
      node.append_tab(tab);
    }
  }
}

#[derive(Event, new, Clone, Copy)]
pub struct RemoveUiEvent(Entity);

impl RemoveUiEvent {
  pub fn on_event(mut events: EventReader<Self>, mut commands: Commands) {
    for event in events.read() {
      let RemoveUiEvent(tab) = *event;
      commands.entity(tab).despawn();
    }
  }
}
