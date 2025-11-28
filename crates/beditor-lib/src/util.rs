pub mod log;
pub mod storage;
pub mod vfs;

use bevy::{
  camera::visibility::{Layer as CameraLayer, RenderLayers},
  ecs::{bundle::NoBundleEffect, system::SystemParam},
  prelude::*,
  reflect::{GetTypeRegistration, TypeRegistration},
  window::{CursorGrabMode, CursorIcon, CursorOptions},
};

#[macro_export]
macro_rules! here {
  () => {{
    use std::io::Write;
    println!("{}({})", file!(), line!());
    std::io::stdout().flush().ok();
  }};

  ($($arg:tt)*) => {{
    use std::io::Write;
    print!("{}({}): ", file!(), line!());
    std::io::stdout().flush().ok();
    println!($($arg)*);
  }};
}

pub fn short_name_of<T>() -> &'static str
where
  T: GetTypeRegistration,
{
  short_name_of_type(&T::get_type_registration())
}

pub fn short_name_of_type(registration: &TypeRegistration) -> &'static str {
  registration.type_info().type_path_table().short_path()
}

pub fn show_cursor(cursor: &mut CursorOptions) {
  cursor.visible = true;
  cursor.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(cursor: &mut CursorOptions) {
  cursor.visible = false;
  cursor.grab_mode = CursorGrabMode::Locked;
}

pub fn set_cursor_icon(commands: &mut Commands, entity: Entity, cursor: impl Into<CursorIcon>) {
  commands.entity(entity).insert(cursor.into());
}

#[allow(unused)]
pub trait WindowExtensions {
  fn center(&self) -> [f32; 2];
}

impl WindowExtensions for Window {
  fn center(&self) -> [f32; 2] {
    [self.width() / 2.0, self.height() / 2.0]
  }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct GameRenderLayer(CameraLayer);

#[derive(Component)]
#[require(RenderLayers = RenderLayers::layer(0))]
pub struct GameEntity;

#[derive(SystemParam)]
pub struct EntityManager<'w, 's> {
  commands: Commands<'w, 's>,
  render_layer: Res<'w, GameRenderLayer>,
}

impl EntityManager<'_, '_> {
  pub fn spawn(&mut self, bundle: impl Bundle) -> EntityCommands<'_> {
    let mut cmds = self
      .commands
      .spawn(RenderLayers::layer(self.render_layer.0));
    cmds.insert(bundle);
    cmds
  }

  pub fn spawn_batch<I>(&mut self, batch: I)
  where
    I: IntoIterator + Send + Sync + 'static,
    I::IntoIter: Send + Sync + 'static,
    I::Item: Bundle<Effect: NoBundleEffect>,
  {
    self
      .commands
      .spawn_batch(batch.into_iter().map(|bundle| (GameEntity, bundle)));
  }
}
