use crate::{misc::ShrinkableViewport, ui::Ui, view::EditorCamera};
use bevy::{ecs::system::SystemParam, prelude::*};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorView {
  viewport_rect: Rect,
}

impl ShrinkableViewport for EditorView {
  type Marker = EditorCamera;

  fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  q_cameras: Query<'w, 's, &'static mut Camera, With<EditorCamera>>,
}

impl Ui for EditorView {
  const NAME: &str = "Editor View";
  const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

  type Params<'w, 's> = Params<'w, 's>;

  fn init(app: &mut App) {
    app.add_systems(PostUpdate, Self::set_viewport);
  }

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = false;
    }
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    let egui_rect = ui.clip_rect();
    self.viewport_rect = Rect {
      max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
      min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
    };
  }

  fn when_rendered(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = true;
    }
  }

  fn when_not_rendered(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = false;
    }
  }

  fn can_clear(&self, _params: Self::Params<'_, '_>) -> bool {
    false
  }

  fn unique() -> bool {
    true
  }

  fn popout() -> bool {
    false
  }
}
