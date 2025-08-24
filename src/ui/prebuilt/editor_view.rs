use crate::{
  ui::{Ui, misc::UiInfo},
  view::EditorCamera,
};
use bevy::{ecs::system::SystemParam, prelude::*, render::camera::Viewport, window::PrimaryWindow};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorView {
  viewport_rect: Rect,
}

impl EditorView {
  pub fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }

  // TODO somewhere in here the physical <-> logical pixel values became wrong (with a grain of salt) in the 0.16 migration
  // fix to enable the viewport resize again
  fn set_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    egui_settings: Single<&bevy_egui::EguiContextSettings>,
    q_editor_views: Query<(&Self, &UiInfo)>,
    mut q_cameras: Query<&mut Camera, With<EditorCamera>>,
  ) {
    for (editor_view, ui_info) in &q_editor_views {
      if ui_info.rendered() {
        for mut camera in &mut q_cameras {
          let scale_factor = window.scale_factor() * egui_settings.scale_factor;

          let viewport = editor_view.viewport();
          let viewport_pos = viewport.left_top().to_vec2() * scale_factor;
          let viewport_size = viewport.size() * scale_factor;

          let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
          let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

          // The desired viewport rectangle at its offset in "physical pixel space"
          let rect = physical_position + physical_size;

          let window_size = window.physical_size();
          if rect.x <= window_size.x && rect.y <= window_size.y {
            let depth = camera
              .viewport
              .as_ref()
              .map(|vp| vp.depth.clone())
              .unwrap_or(0.0..1.0);

            camera.viewport = Some(Viewport {
              physical_position,
              physical_size,
              depth,
            });
          }
        }
      }
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
