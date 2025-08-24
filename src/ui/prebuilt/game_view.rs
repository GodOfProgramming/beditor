use crate::ui::{Ui, misc::UiInfo};
use bevy::{ecs::system::SystemParam, prelude::*, render::camera::Viewport, window::PrimaryWindow};
use persistent_id::Identifiable;
use std::marker::PhantomData;

#[derive(Component, Reflect)]
pub struct GameView<C>
where
  C: Component + Reflect,
{
  viewport_rect: Rect,
  was_rendered: bool,
  #[reflect(ignore)]
  _pd: PhantomData<C>,
}

impl<C> Default for GameView<C>
where
  C: Component + Reflect,
{
  fn default() -> Self {
    Self {
      viewport_rect: default(),
      was_rendered: false,
      _pd: PhantomData,
    }
  }
}

impl<C> GameView<C>
where
  C: Component + Reflect,
{
  pub fn viewport(&self) -> egui::Rect {
    egui::Rect {
      max: egui::Pos2::new(self.viewport_rect.max.x, self.viewport_rect.max.y),
      min: egui::Pos2::new(self.viewport_rect.min.x, self.viewport_rect.min.y),
    }
  }

  fn on_preupdate(mut q_game_views: Query<&mut Self>) {
    for mut game_view in &mut q_game_views {
      game_view.was_rendered = false;
    }
  }

  fn set_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    egui_settings: Single<&bevy_egui::EguiContextSettings>,
    q_game_views: Query<(&Self, &UiInfo)>,
    mut q_cameras: Query<&mut Camera, With<C>>,
  ) {
    for (game_view, ui_info) in &q_game_views {
      if ui_info.rendered() {
        for mut camera in &mut q_cameras {
          let scale_factor = window.scale_factor() * egui_settings.scale_factor;

          let viewport = game_view.viewport();
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
pub struct Params<'w, 's, C: Component> {
  q_cameras: Query<'w, 's, &'static mut Camera, With<C>>,
  title: Local<'s, String>,
}

impl<C> Ui for GameView<C>
where
  C: Component + Reflect + TypePath + Identifiable,
{
  const NAME: &str = <C as Identifiable>::TYPE_NAME;
  const ID: uuid::Uuid = <C as Identifiable>::ID;

  type Params<'w, 's> = Params<'w, 's, C>;

  fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
    params.title.as_str().into()
  }

  fn init(app: &mut App) {
    app
      .add_systems(PreUpdate, Self::on_preupdate)
      .add_systems(PostUpdate, Self::set_viewport);
  }

  fn spawn(mut params: Self::Params<'_, '_>) -> Self {
    let type_path = C::type_path();
    *params.title = format!("Game View of {type_path}");
    default()
  }

  fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
    for mut camera in &mut params.q_cameras {
      camera.is_active = false;
    }
  }

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    self.was_rendered = true;

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
