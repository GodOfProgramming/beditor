use crate::ui::{InspectorSelection, RawUi, SelectedEntities};
use bevy::prelude::*;
use bevy_inspector_egui::bevy_inspector;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Hierarchy;

impl RawUi for Hierarchy {
  const NAME: &str = stringify!(Hierarchy);
  const ID: Uuid = uuid!("860ac319-5c6e-4a2e-83ae-8bb0000d5cb4");

  fn init(app: &mut App) {
    app
      .add_message::<SelectEntityMessage>()
      .add_message::<ReparentMessage>()
      .add_systems(
        FixedUpdate,
        (SelectEntityMessage::handle, ReparentMessage::handle),
      );
  }

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
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
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();

    let ctx_menu = &mut Self::context_menu;
    let mut hierarchy = bevy_inspector::hierarchy::Hierarchy {
      world,
      type_registry: &type_registry,
      selected,
      context_menu: Some(ctx_menu),
      shortcircuit_entity: None,
      extra_state: &mut (),
    };

    hierarchy.show_with_default_filter::<()>(ui)
  }

  fn context_menu(ui: &mut egui::Ui, entity: Entity, world: &mut World, _: &mut ()) {
    if ui.button("Despawn").clicked() {
      world.despawn(entity);
    }

    if ui.button("Select").clicked() {
      world.write_message(SelectEntityMessage(entity));
    }

    if ui.button("Reparent Selected").clicked() {
      world.write_message(ReparentMessage(entity));
    }

    let mut entity_ref = world.entity_mut(entity);

    if entity_ref.get::<ChildOf>().is_some() && ui.button("Remove Parent").clicked() {
      entity_ref.remove::<ChildOf>();
    }
  }
}

#[derive(Message)]
struct SelectEntityMessage(Entity);

impl SelectEntityMessage {
  fn handle(mut message_reader: MessageReader<Self>, mut selection: ResMut<InspectorSelection>) {
    for msg in message_reader.read() {
      select_entity(&mut selection, msg.0);
    }
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
        select_entity(&mut selection, msg.0);
      }
    }
  }
}

fn select_entity(selection: &mut InspectorSelection, entity: Entity) {
  let mut entities = SelectedEntities::default();
  entities.0.select_replace(entity);
  *selection = InspectorSelection::Entities(entities)
}
