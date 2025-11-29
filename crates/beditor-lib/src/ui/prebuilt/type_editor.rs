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

  const HIDDEN: bool = true;

  const REOPEN_ON_STARTUP: bool = false;

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    Self
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    let Some(arc) = world
      .entity(entity)
      .get::<EditingType>()
      .map(|t| Arc::clone(&t.0))
    else {
      return;
    };

    let m = arc.lock();

    let mut tuple = m.borrow_mut();

    ui.heading(&tuple.0);

    ui.separator();

    bevy_inspector_egui::bevy_inspector::ui_for_value(&mut *tuple.1, ui, world);
  }
}

#[derive(Component)]
struct EditingType(Arc<Mutex<RefCell<(String, Box<dyn Reflect>)>>>);

impl EditingType {
  fn new(label: String, value: Box<dyn Reflect>) -> Self {
    Self(Arc::new(Mutex::new(RefCell::new((label, value)))))
  }
}

#[derive(new, Message)]
pub struct OpenTypeEditor(String, Box<dyn Reflect>);

impl Command for OpenTypeEditor {
  fn apply(self, world: &mut World) -> () {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      let entity = ui_manager.spawn_type::<TypeEditor>(world);
      world
        .entity_mut(entity)
        .insert(EditingType::new(self.0, self.1));
      ui_manager.add_tab_to_focused(entity);
    });
  }
}
