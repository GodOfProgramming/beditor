pub mod assets;
mod input;
mod registry;
mod ui;
mod util;
mod view;

use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
use bevy::{
  app::PluginGroupBuilder,
  diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
  },
  ecs::{entity_disabling::Disabled, query::QueryFilter, system::NonSendMarker},
  log::LogPlugin,
  prelude::*,
  reflect::GetTypeRegistration,
  remote::{RemotePlugin, http::RemoteHttpPlugin},
  window::{CursorOptions, PrimaryWindow, WindowCloseRequested, WindowMode},
  winit::WINIT_WINDOWS,
};
use input::InputPlugin;
pub use prelude::*;
use registry::components::{ComponentRegistry, RegisterableComponent, RegisterableComponents};
use serde::{Deserialize, Serialize};
use ui::{UiPlugin, managers::UiManager, prebuilt::game_view::GameView};
use util::{LogLevel, LoggingExtensionsPlugin};
use view::EditorViewPlugin;

use crate::ui::InspectorIntegrationPlugin;

pub mod prelude {
  pub use super::Editor;
  pub use crate::{
    ui::{RawUi, Ui, misc},
    util::{
      EntityManager, GameEntity, GameRenderLayer,
      storage::{Layouts, Settings, Storage},
    },
  };
  pub use bevy_egui;
  pub use macros::{self, Identifiable};
  pub use persistent_id::{self, Identifiable};
  pub use serde;
  pub use uuid;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
  Editing,
  Testing,
  Exiting,
}

#[derive(Deref, DerefMut)]
pub struct Editor {
  #[deref]
  app: App,
  prefab_registrar: PrefabRegistrar,
  ui_manager: UiManager,
  component_registry: ComponentRegistry,
}

impl Default for Editor {
  fn default() -> Self {
    Self::from(App::new())
  }
}

impl Editor {
  pub fn configure_defaults(f: impl FnOnce(DefaultPlugins) -> PluginGroupBuilder) -> Self {
    let mut app = App::new();

    Self::init_app(&mut app, Some(f));

    let ui_manager = UiManager::new(&mut app);

    Self {
      app,
      prefab_registrar: default(),
      component_registry: default(),
      ui_manager,
    }
  }

  pub fn register_component<T: RegisterableComponent>(&mut self) -> &mut Self {
    T::register(self.app.world_mut(), &mut self.component_registry);
    self.register_type::<T>();
    self
  }

  pub fn register_components<T: RegisterableComponents>(&mut self) -> &mut Self {
    T::register_components(self.app.world_mut(), &mut self.component_registry);
    T::register_types(self);
    self
  }

  pub fn register_game_camera<C>(&mut self) -> &mut Self
  where
    C: Component + Reflect + TypePath + Identifiable,
  {
    view::add_game_camera::<C>(&mut self.app);
    self.register_ui::<GameView<C>>()
  }

  pub fn register_ui<U: RawUi>(&mut self) -> &mut Self {
    self.ui_manager.register::<U>(&mut self.app);
    self.register_type::<U>();
    self
  }

  pub fn register_static_prefab<T>(&mut self) -> &mut Self
  where
    T: StaticPrefab,
  {
    self.register_type::<T>();

    self.prefab_registrar.register::<T>();

    self
  }

  pub fn load_prefabs<T>(&mut self) -> &mut Self
  where
    T: Prefab,
  {
    self.register_type::<T>();
    self.app.add_plugins(PrefabPlugin::<T>::default());
    self
  }

  pub fn register_pickable<F: QueryFilter + Send + Sync + 'static>(&mut self) -> &mut Self {
    self
      .app
      .add_plugins(InspectorIntegrationPlugin::<F>::default());
    self
  }

  fn register_type<T>(&mut self)
  where
    T: GetTypeRegistration,
  {
    self.app.register_type::<T>();
  }

  pub fn to_app(self) -> App {
    let Self {
      mut app,
      prefab_registrar,
      ui_manager,
      component_registry,
    } = self;

    app
      .insert_resource(prefab_registrar)
      .insert_resource(component_registry)
      .insert_resource(ui_manager);

    app
  }

  pub fn run(self) -> AppExit {
    self.to_app().run()
  }

  fn init_app<F>(app: &mut App, inspector_fn: Option<F>)
  where
    F: FnOnce(DefaultPlugins) -> PluginGroupBuilder,
  {
    dotenvy::dotenv().ok();

    let default_plugins = DefaultPlugins;

    let default_plugins = if let Some(inspector_fn) = inspector_fn {
      (inspector_fn)(default_plugins)
    } else {
      default_plugins.build()
    };

    app
      .insert_resource(Storage::new().unwrap())
      .init_resource::<EditorSettings>()
      .init_resource::<GameRenderLayer>()
      .add_plugins((
        LoggingExtensionsPlugin,
        default_plugins
          .set(WindowPlugin {
            primary_window: Some(Window {
              title: String::from("Beditor"),
              mode: WindowMode::Windowed,
              visible: false,
              ..default()
            }),
            close_when_requested: false,
            ..default()
          })
          .set(LogPlugin {
            level: LogLevel::Trace.into(),
            custom_layer: util::dynamic_log_layer,
            ..default()
          }),
        MeshPickingPlugin,
        FrameTimeDiagnosticsPlugin::default(),
        EntityCountDiagnosticsPlugin::default(),
        SystemInformationDiagnosticsPlugin,
        RemotePlugin::default(),
        RemoteHttpPlugin::default(),
      ))
      .add_plugins((EditorViewPlugin, InputPlugin, UiPlugin))
      .insert_state(EditorState::Editing)
      .configure_sets(
        Update,
        (
          EditorGlobalSystems,
          EditingSystems
            .in_set(EditorGlobalSystems)
            .run_if(in_state(EditorState::Editing)),
        ),
      )
      .configure_sets(
        FixedUpdate,
        (
          EditorGlobalSystems,
          EditingSystems
            .in_set(EditorGlobalSystems)
            .run_if(in_state(EditorState::Editing)),
        ),
      )
      .add_observer(EnableGameUiEvent::handle)
      .add_observer(DisableGameUiEvent::handle)
      .add_systems(
        Startup,
        (
          configure_windows,
          set_picking_settings,
          initialize_prefabs,
          auto_register_components,
          load_editor_settings,
        ),
      )
      .add_systems(PostStartup, show_window)
      .add_systems(OnEnter(EditorState::Editing), show_window_cursor)
      .add_systems(FixedUpdate, (on_close_requested, handle_window_events))
      .add_systems(
        OnEnter(EditorState::Exiting),
        (save_editor_settings, finish_exit).in_set(EditorGlobalSystems),
      );
  }
}

impl From<App> for Editor {
  fn from(mut app: App) -> Self {
    Self::init_app::<fn(DefaultPlugins) -> PluginGroupBuilder>(&mut app, None);

    let ui_manager = UiManager::new(&mut app);

    Self {
      app,
      prefab_registrar: default(),
      component_registry: default(),
      ui_manager,
    }
  }
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorGlobalSystems;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditingSystems;

#[derive(Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource, Default)]
struct EditorSettings {
  render_ui: bool,
  game_requires_mouse: bool,
  game_requires_picking: bool,
}

impl Default for EditorSettings {
  fn default() -> Self {
    Self {
      render_ui: true,
      game_requires_mouse: false,
      game_requires_picking: false,
    }
  }
}

struct EditorSettingsSetting;

impl AsRef<str> for EditorSettingsSetting {
  fn as_ref(&self) -> &str {
    "editor.settings"
  }
}

fn save_editor_settings(mut settings: Settings, editor_settings: Res<EditorSettings>) -> Result {
  settings.set(EditorSettingsSetting, &*editor_settings)
}

fn load_editor_settings(mut settings: Settings, mut editor_settings: ResMut<EditorSettings>) {
  *editor_settings = settings.get_or_default(EditorSettingsSetting);
}

struct WindowMaximizedSetting;

impl AsRef<str> for WindowMaximizedSetting {
  fn as_ref(&self) -> &str {
    "window.maximized"
  }
}

fn auto_register_components(world: &mut World) {
  world.resource_scope(|world, mut component_registry: Mut<ComponentRegistry>| {
    let app_type_registry = world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = app_type_registry.read();

    for entry in type_registry.iter() {
      component_registry.register_raw(world, entry);
    }
  });
}

fn set_picking_settings(mut picking_settings: ResMut<MeshPickingSettings>) {
  picking_settings.require_markers = true;
}

fn show_window(mut q_windows: Query<&mut Window>) {
  for mut window in &mut q_windows {
    window.visible = true;
  }
}

fn finish_exit(mut app_exit: MessageWriter<AppExit>) {
  app_exit.write(AppExit::Success);
}

fn show_window_cursor(mut q_cursors: Query<&mut CursorOptions>) {
  for mut cursor in &mut q_cursors {
    util::show_cursor(&mut cursor);
  }
}

fn configure_windows(
  mut settings: Settings,
  mut window: Single<&mut Window, With<PrimaryWindow>>,
) -> Result<()> {
  let maximized = settings.get_or_default::<bool>(WindowMaximizedSetting);
  window.set_maximized(maximized);
  Ok(())
}

fn initialize_prefabs(world: &mut World) {
  let Some(registrar) = world.remove_resource::<PrefabRegistrar>() else {
    return;
  };

  let prefabs = Prefabs::new(world, registrar);

  world.insert_resource(prefabs);
}

#[derive(Event)]
pub struct EnableGameUiEvent;

impl EnableGameUiEvent {
  /// This will likely need further logic to account for game logic that wants to disable UI but needs to not be managed by this
  /// Maybe a marker component that signals that the editor should not disable or enable the UI as a first pass
  fn handle(
    _: On<Self>,
    mut commands: Commands,
    q_ui: Query<Entity, (With<Node>, Allow<Disabled>)>,
  ) {
    for entity in q_ui {
      commands
        .entity(entity)
        .remove_recursive::<Children, Disabled>();
    }
  }
}

#[derive(Event)]
pub struct DisableGameUiEvent;

impl DisableGameUiEvent {
  fn handle(_: On<Self>, mut commands: Commands, q_ui: Query<Entity, With<Node>>) {
    for entity in q_ui {
      commands
        .entity(entity)
        .insert_recursive::<Children>(Disabled);
    }
  }
}

fn on_close_requested(
  mut close_requests: MessageReader<WindowCloseRequested>,
  mut next_editor_state: ResMut<NextState<EditorState>>,
) {
  if !close_requests.is_empty() {
    close_requests.clear();
    next_editor_state.set(EditorState::Exiting)
  }
}

fn handle_window_events(
  mut settings: Settings,
  mut events: MessageReader<bevy::window::WindowResized>,
  _non_send_marker: NonSendMarker,
) -> Result {
  WINIT_WINDOWS.with_borrow(|windows| {
    for event in events.read() {
      let Some(winit_window) = windows.get_window(event.window) else {
        continue;
      };

      settings.set(WindowMaximizedSetting, winit_window.is_maximized())?;
    }

    Ok(())
  })
}
