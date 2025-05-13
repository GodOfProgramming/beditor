use beditor::{Editor, Ui, misc::NoParams};
use bevy::prelude::*;
use egui_demo_lib::{View, WidgetGallery};
use uuid::uuid;

fn main() {
  let mut editor = Editor::default();

  editor.register_ui::<CustomPanel>();

  editor.run();
}

#[derive(Reflect, Component, Default)]
struct CustomPanel(#[reflect(ignore)] WidgetGallery);

impl Ui for CustomPanel {
  const NAME: &str = "Custom Panel";

  const ID: uuid::Uuid = uuid!("b2d3a7ea-a68c-4788-a9e5-16b51d94ce52");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self::default()
  }

  fn render(&mut self, ui: &mut bevy_egui::egui::Ui, _params: Self::Params<'_, '_>) {
    self.0.ui(ui);
  }
}
