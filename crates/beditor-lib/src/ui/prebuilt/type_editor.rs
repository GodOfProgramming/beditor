use std::{cell::RefCell, sync::Arc};

use crate::{RawUi, ui::managers::UiManager};
use bevy::prelude::*;
use derive_new::new;
use parking_lot::Mutex;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct TypeEditor;

impl RawUi for TypeEditor {
  const NAME: &str = stringify!(TypeEditor);

  const ID: Uuid = uuid!("2b01d041-d8b3-4cbe-8ca7-f6ae8e8ef7dd");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    Self
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    ui.heading("");

    ui.separator();

    let Some(type_arc) = world
      .entity(entity)
      .get::<EditingType>()
      .map(|t| Arc::clone(&t.0))
    else {
      return;
    };

    let type_mutex = type_arc.lock();

    let mut reflected_type = type_mutex.borrow_mut();

    bevy_inspector_egui::bevy_inspector::ui_for_value(&mut **reflected_type, ui, world);
  }

  fn unique() -> bool {
    false
  }

  fn closeable(_entity: Entity, _world: &mut World) -> bool {
    true
  }
}

#[derive(Component)]
struct EditingType(Arc<Mutex<RefCell<Box<dyn Reflect>>>>);

impl EditingType {
  fn new(value: Box<dyn Reflect>) -> Self {
    Self(Arc::new(Mutex::new(RefCell::new(value))))
  }
}

#[derive(new, Message)]
pub struct OpenTypeEditor(Box<dyn Reflect>);

impl Command for OpenTypeEditor {
  fn apply(self, world: &mut World) -> () {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      let entity = ui_manager.spawn_type::<TypeEditor>(world);
      world.entity_mut(entity).insert(EditingType::new(self.0));
      ui_manager.add_tab_to_focused(entity);
    });
  }
}
