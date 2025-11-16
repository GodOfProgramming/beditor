use crate::{
  ui::Ui,
  util::{ChangeLogLevelEvent, LogLevel, LogLevelChangedEvent},
  view::{RenderCameras, SyncRenderCamerasEvent},
};
use bevy::{diagnostic::DiagnosticsStore, ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContext, egui};
use bevy_inspector_egui::reflect_inspector::ui_for_value;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct DebugMenu {
  ui_debug_on_hover: bool,
  log_level: LogLevel,
}

impl DebugMenu {
  fn log_level_selector(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.push_id("log-level-selector", |ui| {
      ui.horizontal(|ui| {
        let type_registry = params.type_registry.as_ref().read();

        ui.label("Log Level");
        let mut log_level = self.log_level;
        if ui_for_value(&mut log_level, ui, &type_registry) {
          params.commands.trigger(ChangeLogLevelEvent::new(log_level));
        }
      });
    });
  }

  fn diagnostics(&self, ui: &mut egui::Ui, params: &Params) {
    egui::Grid::new("sys-diagnostics").show(ui, |ui| {
      for diagnostic in params.diagnostics.iter() {
        ui.label(diagnostic.path().as_str());
        if let Some(average) = diagnostic.average() {
          ui.label(format!("{:.2}", average));
        }
        ui.end_row();
      }
    });
  }

  fn handle_ui_debug(event: On<DebugUiEvent>, mut q_egui_ctx: Query<&mut EguiContext>) {
    for mut ctx in &mut q_egui_ctx {
      let ctx = ctx.get_mut();
      ctx.set_debug_on_hover(event.0);
    }
  }

  fn handle_log_level_changes(event: On<LogLevelChangedEvent>, mut q_self: Query<&mut Self>) {
    for mut this in &mut q_self {
      this.log_level = **event;
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  commands: Commands<'w, 's>,
  type_registry: Res<'w, AppTypeRegistry>,
  diagnostics: Res<'w, DiagnosticsStore>,
  render_cameras: ResMut<'w, RenderCameras>,
}

impl Ui for DebugMenu {
  const NAME: &str = "Debug Menu";
  const ID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

  type Params<'w, 's> = Params<'w, 's>;

  fn init(app: &mut App) {
    app
      .add_observer(Self::handle_ui_debug)
      .add_observer(Self::handle_log_level_changes);
  }

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self::default()
  }

  fn unique() -> bool {
    true
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    self.diagnostics(ui, &params);

    ui.separator();

    self.log_level_selector(ui, &mut params);

    ui.separator();

    if ui
      .checkbox(&mut self.ui_debug_on_hover, "Debug UI")
      .clicked()
    {
      params
        .commands
        .trigger(DebugUiEvent(self.ui_debug_on_hover));
    }

    if ui
      .checkbox(&mut **params.render_cameras, "Render Cameras")
      .clicked()
    {
      params.commands.trigger(SyncRenderCamerasEvent);
    }
  }
}

#[derive(Event)]
struct DebugUiEvent(bool);
