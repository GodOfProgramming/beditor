pub mod storage;
pub mod vfs;

use crate::util::storage::Settings;
use bevy::{
  camera::visibility::{Layer as CameraLayer, RenderLayers},
  ecs::{
    bundle::NoBundleEffect,
    system::{SystemParam, SystemState},
  },
  log::{
    BoxedLayer, Level,
    tracing::{self, Subscriber},
    tracing_subscriber::{
      self, Layer,
      layer::{self, Filter, Layered, SubscriberExt},
      registry::LookupSpan,
      reload,
    },
  },
  prelude::*,
  reflect::GetTypeRegistration,
  window::{CursorGrabMode, CursorIcon, CursorOptions},
};
use derive_new::new;
use egui_tracing::EventCollector;
use parking_lot::Mutex;
use profiling::tracing::level_filters::LevelFilter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

#[derive(Resource, Deref, DerefMut, Clone)]
pub struct LogHandle(reload::Handle<LevelFilter, tracing_subscriber::Registry>);

struct LevelFilterWrapper<S: Subscriber>(reload::Layer<LevelFilter, S>);

impl<S: Subscriber> Filter<S> for LevelFilterWrapper<S> {
  fn enabled(&self, meta: &tracing::Metadata<'_>, cx: &layer::Context<'_, S>) -> bool {
    layer::Layer::enabled(&self.0, meta, cx.clone())
  }

  fn callsite_enabled(
    &self,
    meta: &'static tracing::Metadata<'static>,
  ) -> tracing::subscriber::Interest {
    layer::Filter::callsite_enabled(&self.0, meta)
  }

  #[inline]
  fn event_enabled(&self, event: &tracing::Event<'_>, cx: &layer::Context<'_, S>) -> bool {
    layer::Layer::<S>::event_enabled(&self.0, event, cx.clone())
  }

  fn max_level_hint(&self) -> Option<LevelFilter> {
    layer::Layer::<S>::max_level_hint(&self.0)
  }

  fn on_new_span(
    &self,
    attrs: &tracing::span::Attributes<'_>,
    id: &tracing::span::Id,
    ctx: layer::Context<'_, S>,
  ) {
    layer::Layer::<S>::on_new_span(&self.0, attrs, id, ctx)
  }

  fn on_record(
    &self,
    id: &tracing::span::Id,
    values: &tracing::span::Record<'_>,
    ctx: layer::Context<'_, S>,
  ) {
    layer::Layer::<S>::on_record(&self.0, id, values, ctx)
  }

  fn on_enter(&self, id: &tracing::span::Id, ctx: layer::Context<'_, S>) {
    layer::Layer::<S>::on_enter(&self.0, id, ctx)
  }

  fn on_exit(&self, id: &tracing::span::Id, ctx: layer::Context<'_, S>) {
    layer::Layer::<S>::on_exit(&self.0, id, ctx)
  }

  fn on_close(&self, id: tracing::span::Id, ctx: layer::Context<'_, S>) {
    layer::Layer::<S>::on_close(&self.0, id, ctx)
  }
}

#[derive(Resource, Deref, DerefMut, Clone)]
pub struct LogCollector(Arc<Mutex<EventCollector>>);

impl<S> Layer<S> for LogCollector
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  fn on_event(&self, event: &tracing::Event<'_>, ctx: layer::Context<'_, S>) {
    self.0.lock().on_event(event, ctx);
  }
}

pub fn dynamic_log_layer(app: &mut App) -> Option<BoxedLayer> {
  let mut system_state = SystemState::<Settings>::new(app.world_mut());
  let mut settings = system_state.get_mut(app.world_mut());
  let level = settings.get_or_default::<LogLevel>(LogLevelSetting);

  let (reload_layer, handle) = reload::Layer::new(level.into());
  app.insert_resource(LogHandle(handle));
  app.world_mut().trigger(LogLevelChangedEvent(level));

  let collector = EventCollector::default().with_level(level.into());
  let shared_collector = Arc::new(Mutex::new(collector));
  let log_collector = LogCollector(Arc::clone(&shared_collector));
  app.insert_resource(log_collector);

  // Cannot use this yet:
  // https://github.com/tokio-rs/tracing/issues/2704
  // let layer = log_collector.with_filter(LevelFilterWrapper(reload_layer));

  Some(reload_layer.boxed())
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
