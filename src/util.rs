pub mod vfs;

use crate::cache::{Cache, Saveable};
use bevy::{
  log::{
    BoxedLayer, Level,
    tracing_subscriber::{self, Layer, reload},
  },
  platform::collections::HashMap,
  prelude::*,
  reflect::GetTypeRegistration,
  window::CursorGrabMode,
  winit::cursor::CursorIcon,
};
use profiling::tracing::level_filters::LevelFilter;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;

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

pub fn show_cursor(window: &mut Window) {
  window.cursor_options.visible = true;
  window.cursor_options.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(window: &mut Window) {
  window.cursor_options.visible = false;
  window.cursor_options.grab_mode = CursorGrabMode::Locked;
}

pub fn set_cursor_icon(commands: &mut Commands, entity: Entity, cursor: impl Into<CursorIcon>) {
  commands.entity(entity).insert(cursor.into());
}

pub fn sorted_keys<S, K: Ord + Serialize, V: Serialize>(
  value: &HashMap<K, V>,
  serializer: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  let ordered: BTreeMap<_, _> = value.iter().collect();
  ordered.serialize(serializer)
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct LogInfo {
  level: LogLevel,
}

impl Saveable for LogInfo {
  const KEY: &str = "logging";
}

impl LogInfo {
  pub fn on_app_exit(logging_settings: Res<LoggingSettings>, mut cache: ResMut<Cache>) {
    cache.store(&LogInfo {
      level: logging_settings.level,
    });
  }
}

#[derive(Resource)]
pub struct LoggingSettings {
  level: LogLevel,
  filter_handle: reload::Handle<LevelFilter, tracing_subscriber::Registry>,
}

impl LoggingSettings {
  pub fn level(&self) -> LogLevel {
    self.level
  }

  pub fn set_level(&mut self, level: LogLevel) {
    self.level = level;
    self
      .filter_handle
      .modify(|filter| *filter = level.into())
      .inspect_err(|err| {
        eprintln!("Failed to set log level filter: {err}");
      })
      .ok();
  }

  pub fn restore(mut logging: ResMut<Self>, cache: Res<Cache>) {
    let Some(log_info) = cache.get::<LogInfo>() else {
      error!("Failed to get log info, using default logging settings");
      return;
    };

    logging.set_level(log_info.level);
  }
}

pub fn dynamic_log_layer(app: &mut App) -> Option<BoxedLayer> {
  let level = LogLevel::Info;
  let (filter, handle) = reload::Layer::new(level.into());
  app.insert_resource(LoggingSettings {
    level,
    filter_handle: handle,
  });

  Some(filter.boxed())
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
