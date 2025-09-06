pub mod settings;
pub mod vfs;

use crate::{
  cache::{Cache, Saveable},
  util::settings::Settings,
};
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
use derive_new::new;
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

pub struct LoggingExtensionsPlugin;

impl Plugin for LoggingExtensionsPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_event::<ChangeLogLevelEvent>()
      .add_event::<LogLevelChangedEvent>();
  }
}

#[derive(Event, Deref, DerefMut, new)]
pub struct ChangeLogLevelEvent(LogLevel);

impl ChangeLogLevelEvent {
  pub fn handle(
    mut events: EventReader<Self>,
    mut settings: ResMut<Settings>,
    log_handle: Res<LogHandle>,
    mut writer: EventWriter<LogLevelChangedEvent>,
  ) -> Result {
    for event in events.read() {
      settings.set(LogLevelSetting, **event)?;
      log_handle
        .modify(|filter| *filter = (**event).into())
        .inspect_err(|err| {
          eprintln!("Failed to set log level filter: {err}");
        })
        .ok();
      LogLevelChangedEvent(**event).fire(&mut writer);
    }

    Ok(())
  }
}

#[derive(Event, Deref, DerefMut, new)]
pub struct LogLevelChangedEvent(LogLevel);

#[derive(Resource, Deref, DerefMut)]
pub struct LogHandle(reload::Handle<LevelFilter, tracing_subscriber::Registry>);

pub fn dynamic_log_layer(app: &mut App) -> Option<BoxedLayer> {
  let mut settings = app.world_mut().resource_mut::<Settings>();
  let level = settings.get_or_default::<LogLevel>(LogLevelSetting);
  let (filter, handle) = reload::Layer::new(level.into());
  app.insert_resource(LogHandle(handle));
  LogLevelChangedEvent(level).fire(app.world_mut());

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

pub trait EventEmitter<E: Event> {
  fn fire(&mut self, event: E);
}

impl<E: Event> EventEmitter<E> for World {
  fn fire(&mut self, event: E) {
    self.send_event(event);
  }
}

impl<E: Event> EventEmitter<E> for EventWriter<'_, E> {
  fn fire(&mut self, event: E) {
    self.write(event);
  }
}

pub trait FireEvent: Event + Sized {
  fn fire<E: EventEmitter<Self>>(self, emitter: &mut E) {
    emitter.fire(self);
  }
}

impl<E: Event + Sized> FireEvent for E {}
