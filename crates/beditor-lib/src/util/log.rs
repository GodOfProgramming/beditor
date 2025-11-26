use crate::Settings;
use bevy::{
  ecs::system::SystemState,
  log::{
    tracing::{self, Level, Subscriber, error, level_filters::LevelFilter},
    tracing_subscriber::{
      self, EnvFilter, Layer,
      filter::{FromEnvError, ParseError},
      layer::{self, SubscriberExt},
      registry::{LookupSpan, Registry},
      reload,
    },
  },
  prelude::*,
};
use core::error::Error;
use derive_new::new;
use egui_tracing::EventCollector;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{any::TypeId, sync::Arc};
use tracing::{Dispatch, Metadata, span, subscriber::Interest};
use tracing_log::LogTracer;

pub const DEFAULT_FILTER: &str = "wgpu=error,naga=warn";

type ReloadHandle = reload::Handle<LevelFilter, Registry>;

#[derive(Default)]
pub struct LogPlugin;

impl Plugin for LogPlugin {
  #[expect(clippy::print_stderr, reason = "Allowed during logger setup")]
  fn build(&self, app: &mut App) {
    let mut system_state = SystemState::<Settings>::new(app.world_mut());
    let mut settings = system_state.get_mut(app.world_mut());
    let level = settings.get_or_default::<LogLevel>(LogLevelSetting);

    app
      .add_observer(ChangeLogLevelEvent::handle.pipe(fire_log_level_changed))
      .add_systems(Startup, (move || level).pipe(fire_log_level_changed));

    let finished_subscriber;
    let subscriber = Registry::default();

    let subscriber = {
      let (reload_layer, handle) = reload::Layer::new(level.into());
      app.insert_resource(LogHandle(handle));
      app.world_mut().trigger(LogLevelChangedEvent(level));

      let collector = EventCollector::default().with_level(level.into());
      let collector_arc = Arc::new(Mutex::new(collector));
      app.insert_resource(EventCollectorHandle(Arc::clone(&collector_arc)));

      let custom_collector = CustomCollector {
        reload_layer,
        event_collector: collector_arc,
      };

      subscriber.with(custom_collector)
    };

    let default_filter = { format!("{},{}", Level::TRACE, DEFAULT_FILTER) };
    let filter_layer = EnvFilter::try_from_default_env()
      .or_else(|from_env_error| {
        _ = from_env_error
          .source()
          .and_then(|source| source.downcast_ref::<ParseError>())
          .map(|parse_err| {
            // we cannot use the `error!` macro here because the logger is not ready yet.
            eprintln!("LogPlugin failed to parse filter from env: {parse_err}");
          });

        Ok::<EnvFilter, FromEnvError>(EnvFilter::builder().parse_lossy(&default_filter))
      })
      .unwrap();
    let subscriber = subscriber.with(filter_layer);

    {
      #[cfg(feature = "profiling")]
      let tracy_layer = tracing_tracy::TracyLayer::default();

      let fmt_layer = tracing_subscriber::fmt::Layer::default().with_writer(std::io::stderr);

      #[cfg(feature = "profiling")]
      let fmt_layer = fmt_layer.with_filter(tracing_subscriber::filter::FilterFn::new(|meta| {
        meta.fields().field("tracy.frame_mark").is_none()
      }));

      let subscriber = subscriber.with(fmt_layer);

      #[cfg(feature = "profiling")]
      let subscriber = subscriber.with(tracy_layer);
      finished_subscriber = subscriber;
    }

    let logger_already_set = LogTracer::init().is_err();
    let subscriber_already_set =
      tracing::subscriber::set_global_default(finished_subscriber).is_err();

    match (logger_already_set, subscriber_already_set) {
      (true, true) => error!(
        "Could not set global logger and tracing subscriber as they are already set. Consider disabling LogPlugin."
      ),
      (true, false) => {
        error!("Could not set global logger as it is already set. Consider disabling LogPlugin.")
      }
      (false, true) => error!(
        "Could not set global tracing subscriber as it is already set. Consider disabling LogPlugin."
      ),
      (false, false) => (),
    }
  }
}

#[derive(new, Event, Deref, DerefMut)]
pub struct ChangeLogLevelEvent(LogLevel);

impl ChangeLogLevelEvent {
  pub fn handle(
    event: On<Self>,
    mut settings: Settings,
    log_handle: Res<LogHandle>,
  ) -> Result<LogLevel> {
    settings.set(LogLevelSetting, **event)?;
    log_handle
      .modify(|filter| *filter = (**event).into())
      .inspect_err(|err| {
        eprintln!("Failed to set log level filter: {err}");
      })
      .ok();

    Ok(**event)
  }
}

fn fire_log_level_changed(level: In<LogLevel>, mut commands: Commands) {
  commands.trigger(LogLevelChangedEvent(*level));
}

#[derive(Event, Deref, DerefMut, new)]
pub struct LogLevelChangedEvent(LogLevel);

#[derive(Resource, Deref, DerefMut, Clone)]
pub struct LogHandle(ReloadHandle);

#[derive(Resource, Deref, DerefMut, Clone)]
pub struct EventCollectorHandle(Arc<Mutex<EventCollector>>);

pub struct CustomCollector<S> {
  reload_layer: reload::Layer<LevelFilter, S>,
  event_collector: Arc<Mutex<EventCollector>>,
}

impl<S> Layer<S> for CustomCollector<S>
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  #[inline]
  fn on_register_dispatch(&self, subscriber: &Dispatch) {
    self.reload_layer.on_register_dispatch(subscriber);
  }

  #[inline]
  fn on_layer(&mut self, subscriber: &mut S) {
    self.reload_layer.on_layer(subscriber);
  }

  #[inline]
  fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
    self.reload_layer.register_callsite(metadata)
  }

  #[inline]
  fn enabled(&self, metadata: &Metadata<'_>, ctx: layer::Context<'_, S>) -> bool {
    self.reload_layer.enabled(metadata, ctx)
  }

  #[inline]
  fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_new_span(attrs, id, ctx)
  }

  #[inline]
  fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_record(span, values, ctx)
  }

  #[inline]
  fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_follows_from(span, follows, ctx)
  }

  #[inline]
  fn event_enabled(&self, event: &tracing::event::Event<'_>, ctx: layer::Context<'_, S>) -> bool {
    self.reload_layer.event_enabled(event, ctx)
  }

  #[inline]
  fn on_event(&self, event: &tracing::event::Event<'_>, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_event(event, ctx.clone());
    self.event_collector.lock().on_event(event, ctx);
  }

  #[inline]
  fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_enter(id, ctx)
  }

  #[inline]
  fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_exit(id, ctx)
  }

  #[inline]
  fn on_close(&self, id: span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_close(id, ctx)
  }

  #[inline]
  fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: layer::Context<'_, S>) {
    self.reload_layer.on_id_change(old, new, ctx)
  }

  #[inline]
  fn max_level_hint(&self) -> Option<LevelFilter> {
    self.reload_layer.max_level_hint()
  }

  #[inline]
  unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
    unsafe { self.reload_layer.downcast_raw(id) }
  }
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
