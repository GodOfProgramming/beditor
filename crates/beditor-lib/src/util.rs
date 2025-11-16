pub mod storage;
pub mod vfs;

use crate::util::storage::Settings;
use bevy::{
  ecs::system::SystemState,
  log::{
    BoxedLayer, Level,
    tracing_subscriber::{self, Layer, reload},
  },
  prelude::*,
  reflect::GetTypeRegistration,
  window::{CursorGrabMode, CursorIcon, CursorOptions},
};
use derive_new::new;
use profiling::tracing::level_filters::LevelFilter;
use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! here {
  () => {{
    use std::io::Write;
    println!("{}({})", file!(), line!());
    std::io::stdout().flush().ok();
  }};
}

pub fn short_name_of<T>() -> &'static str
where
  T: GetTypeRegistration,
{
  T::get_type_registration()
    .type_info()
    .type_path_table()
    .short_path()
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

pub struct LoggingExtensionsPlugin;

impl Plugin for LoggingExtensionsPlugin {
  fn build(&self, app: &mut App) {
    app.add_observer(ChangeLogLevelEvent::handle);
  }
}

#[derive(Event, Deref, DerefMut, new)]
pub struct ChangeLogLevelEvent(LogLevel);

impl ChangeLogLevelEvent {
  pub fn handle(
    event: On<Self>,
    mut commands: Commands,
    mut settings: Settings,
    log_handle: Res<LogHandle>,
  ) -> Result {
    settings.set(LogLevelSetting, **event)?;
    log_handle
      .modify(|filter| *filter = (**event).into())
      .inspect_err(|err| {
        eprintln!("Failed to set log level filter: {err}");
      })
      .ok();

    commands.trigger(LogLevelChangedEvent(**event));

    Ok(())
  }
}

#[derive(Event, Deref, DerefMut, new)]
pub struct LogLevelChangedEvent(LogLevel);

#[derive(Resource, Deref, DerefMut)]
pub struct LogHandle(reload::Handle<LevelFilter, tracing_subscriber::Registry>);

pub fn dynamic_log_layer(app: &mut App) -> Option<BoxedLayer> {
  let mut system_state = SystemState::<Settings>::new(app.world_mut());
  let mut settings = system_state.get_mut(app.world_mut());
  let level = settings.get_or_default::<LogLevel>(LogLevelSetting);
  let (filter, handle) = reload::Layer::new(level.into());
  app.insert_resource(LogHandle(handle));
  app.world_mut().trigger(LogLevelChangedEvent(level));

  Some(filter.boxed())
}

struct LogLevelSetting;

impl AsRef<str> for LogLevelSetting {
  fn as_ref(&self) -> &str {
    "log.level"
  }
}

#[derive(Reflect, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
  Trace,
  Debug,
  #[default]
  Info,
  Warn,
  Error,
}

impl From<LogLevel> for Level {
  fn from(value: LogLevel) -> Self {
    match value {
      LogLevel::Trace => Level::TRACE,
      LogLevel::Debug => Level::DEBUG,
      LogLevel::Info => Level::INFO,
      LogLevel::Warn => Level::WARN,
      LogLevel::Error => Level::ERROR,
    }
  }
}

impl From<LogLevel> for LevelFilter {
  fn from(value: LogLevel) -> Self {
    match value {
      LogLevel::Trace => LevelFilter::TRACE,
      LogLevel::Debug => LevelFilter::DEBUG,
      LogLevel::Info => LevelFilter::INFO,
      LogLevel::Warn => LevelFilter::WARN,
      LogLevel::Error => LevelFilter::ERROR,
    }
  }
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
