pub mod assets;
mod input;
mod registry;
mod ui;
mod util;
mod view;

use crate::util::{ChangeLogLevelEvent, LoggingExtensionsPlugin};
use assets::{Prefab, PrefabPlugin, PrefabRegistrar, Prefabs, StaticPrefab};
use bevy::{
  app::PluginGroupBuilder,
  diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
  },
  log::{DEFAULT_FILTER, LogPlugin},
  picking::hover::PickingInteraction,
  prelude::*,
  reflect::GetTypeRegistration,
  remote::{RemotePlugin, http::RemoteHttpPlugin},
  window::{PrimaryWindow, WindowCloseRequested, WindowMode},
  winit::WinitWindows,
};
use bevy_egui::EguiContext;
use input::InputPlugin;
pub use prelude::*;
use registry::components::{ComponentRegistry, RegisterableComponent, RegisterableComponents};
use ui::{UiPlugin, managers::UiManager, prebuilt::game_view::GameView};
use util::LogLevel;
use view::EditorViewPlugin;

pub mod prelude {
  pub use super::Editor;
  pub use crate::{
    ui::{RawUi, Ui, misc},
    util::storage::{Layouts, Settings, Storage},
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
    Self::new(App::new())
  }
}

impl Editor {
  pub fn new(mut app: App) -> Self {
    Self::init_app::<fn(DefaultPlugins) -> PluginGroupBuilder>(&mut app, None);

    let ui_manager = UiManager::new(&mut app);

    Self {
      app,
      prefab_registrar: default(),
      component_registry: default(),
      ui_manager,
    }
  }

  pub fn new_with_defaults(f: impl FnOnce(DefaultPlugins) -> PluginGroupBuilder) -> Self {
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

  // systems

  fn set_picking_settings(mut picking_settings: ResMut<MeshPickingSettings>) {
    picking_settings.require_markers = true;
  }

  fn show_window(mut q_windows: Query<&mut Window>) {
    for mut window in &mut q_windows {
      window.visible = true;
    }
  }

  fn show_window_cursor(mut q_windows: Query<&mut Window>) {
    for mut window in q_windows.iter_mut() {
      util::show_cursor(&mut window);
    }
  }

  fn startup(
    mut settings: Settings,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
  ) -> Result<()> {
    let maximized = settings.get_or_default::<bool>(WindowMaximizedSetting);

    window.set_maximized(maximized);

    Ok(())
  }

  fn remove_picking_from_targets(
    mut commands: Commands,
    q_targets: Query<Entity, (With<Pickable>, Without<Camera>)>,
  ) {
    for target in q_targets.iter() {
      commands.entity(target).remove::<Pickable>();
      debug!("Removed Pickable from {target}");
    }
  }

  fn initialize_prefabs(world: &mut World) {
    let Some(registrar) = world.remove_resource::<PrefabRegistrar>() else {
      return;
    };

    let prefabs = Prefabs::new(world, registrar);

    world.insert_resource(prefabs);
  }

  #[allow(clippy::type_complexity)]
  fn auto_register_picking_targets(
    mut commands: Commands,
    q_entities: Query<
      (Entity, Option<&Name>),
      (
        Without<Pickable>,
        Or<(With<Sprite>, With<Mesh2d>, With<Mesh3d>)>,
      ),
    >,
  ) {
    for (entity, name) in &q_entities {
      if let Some(name) = name {
        debug!("Registered picking on object: {name}");
      } else {
        debug!("Registered picking on entity: {entity}");
      }

      commands
        .entity(entity)
        .insert(Pickable {
          is_hoverable: true,
          should_block_lower: true,
        })
        .observe(Self::handle_click_event);
    }
  }

  fn handle_click_event(
    trigger: Trigger<Pointer<Click>>,
    mut selection: ResMut<ui::InspectorSelection>,
    mut q_egui: Single<&mut EguiContext>,
    q_pickables: Query<&Pickable>,
  ) {
    let egui_context = q_egui.get_mut();
    let modifiers = egui_context.input(|i| i.modifiers);

    let target = trigger.target;

    debug!("Received pick for {target}");

    if q_pickables.get(target).is_ok() {
      debug!("{target} is not pickable");
      selection.add_selected(target, modifiers.ctrl);
    }
  }

  // TODO this is inefficient, but picking seems to be weird, use the above when it is eventually reliable
  fn pick_all(
    mut selection: ResMut<ui::InspectorSelection>,
    mut q_egui: Single<&mut EguiContext>,
    q_pickables: Query<(Entity, &PickingInteraction), With<Pickable>>,
  ) {
    let egui_context = q_egui.get_mut();
    let modifiers = egui_context.input(|i| i.modifiers);

    for (entity, interaction) in &q_pickables {
      if *interaction != PickingInteraction::Pressed {
        continue;
      }

      debug!("Received pick for {entity}");

      selection.add_selected(entity, modifiers.ctrl);
    }
  }

  fn on_close_requested(
    close_requests: EventReader<WindowCloseRequested>,
    mut next_editor_state: ResMut<NextState<EditorState>>,
  ) {
    if !close_requests.is_empty() {
      next_editor_state.set(EditorState::Exiting)
    }
  }

  fn handle_window_events(
    winit_windows: NonSendMut<WinitWindows>,
    mut settings: Settings,
    mut events: EventReader<bevy::window::WindowResized>,
  ) -> Result {
    for event in events.read() {
      let Some(winit_window) = winit_windows.get_window(event.window) else {
        continue;
      };

      settings.set(WindowMaximizedSetting, winit_window.is_maximized())?;
    }

    Ok(())
  }

  fn init_app<F>(app: &mut App, inspector_fn: Option<F>)
  where
    F: FnOnce(DefaultPlugins) -> PluginGroupBuilder,
  {
    let default_plugins = DefaultPlugins;

    let default_plugins = if let Some(inspector_fn) = inspector_fn {
      (inspector_fn)(default_plugins)
    } else {
      default_plugins.build()
    };

    app
      .insert_resource(Storage::new().unwrap())
      .init_resource::<EditorSettings>()
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
            filter: DEFAULT_FILTER.to_string(),
            custom_layer: util::dynamic_log_layer,
          }),
        EditorViewPlugin,
        MeshPickingPlugin,
        InputPlugin,
        UiPlugin,
        FrameTimeDiagnosticsPlugin::default(),
        EntityCountDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
        RemotePlugin::default(),
        RemoteHttpPlugin::default(),
      ))
      .insert_state(EditorState::Editing)
      .configure_sets(
        Update,
        (
          EditorGlobal,
          Editing
            .in_set(EditorGlobal)
            .run_if(in_state(EditorState::Editing)),
        ),
      )
      .add_systems(
        Startup,
        (
          Self::startup,
          (Self::set_picking_settings, Self::initialize_prefabs),
        ),
      )
      .add_systems(PostStartup, Self::show_window)
      .add_systems(OnEnter(EditorState::Editing), Self::show_window_cursor)
      .add_systems(
        OnExit(EditorState::Editing),
        Self::remove_picking_from_targets,
      )
      .add_systems(FixedUpdate, ChangeLogLevelEvent::handle)
      .add_systems(
        Update,
        (
          (Self::auto_register_picking_targets, Self::pick_all).in_set(Editing),
          Self::on_close_requested,
          Self::handle_window_events,
        ),
      )
      .add_systems(
        OnEnter(EditorState::Exiting),
        (
          (
            view::view2d::save_settings,
            view::view3d::save_settings,
            UiPlugin::on_app_exit,
          ),
          |mut app_exit: EventWriter<AppExit>| {
            app_exit.write(AppExit::Success);
          },
        )
          .chain()
          .in_set(EditorGlobal),
      );
  }
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorGlobal;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct Editing;

#[derive(Resource)]
struct EditorSettings {
  render_ui: bool,
}

impl Default for EditorSettings {
  fn default() -> Self {
    Self { render_ui: true }
  }
}

struct WindowMaximizedSetting;

impl AsRef<str> for WindowMaximizedSetting {
  fn as_ref(&self) -> &str {
    "window.maximized"
  }
}
