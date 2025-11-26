use crate::{
  EditorState, Layouts, Settings, StartEditorInTestingSetting, UiManager,
  misc::{DockExtensions, MissingUi},
  ui::{
    EditorUi, InspectorSelection, components,
    managers::{LayoutManager, SaveLayoutOnExitSetting},
  },
  view::cam::{ActiveEditorCamera, MoveCameraEvent, PointCameraEvent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::EguiPrimaryContextPass;
use egui::TextBuffer;
use egui_dock::DockState;
use persistent_id::PersistentId;
use uuid::Uuid;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  commands: Commands<'w, 's>,

  editor_state: Res<'w, State<EditorState>>,
  next_editor_state: ResMut<'w, NextState<EditorState>>,
  active_camera_state: Res<'w, State<ActiveEditorCamera>>,
  next_active_camera: ResMut<'w, NextState<ActiveEditorCamera>>,
  selection: Res<'w, InspectorSelection>,

  layout_manager: Res<'w, LayoutManager>,

  save_layout_dialog_state: ResMut<'w, SaveLayoutDialogState>,
  reset_layout_dialog_state: ResMut<'w, ResetLayoutDialogState>,

  cached_settings: ResMut<'w, CachedSettings>,
  settings: Settings<'w>,

  q_transforms: Query<'w, 's, &'static Transform>,
}

#[derive(Resource, Reflect, Default)]
struct CachedSettings {
  save_layout_on_exit: bool,
  start_in_testing: bool,
}

#[derive(Resource, Default)]
struct SaveLayoutDialogState {
  open: bool,
  error: Option<String>,
}

#[derive(Resource, Default)]
struct ResetLayoutDialogState {
  open: bool,
}

pub fn init(app: &mut App) {
  app
    .init_resource::<CachedSettings>()
    .init_resource::<SaveLayoutDialogState>()
    .init_resource::<ResetLayoutDialogState>()
    .add_message::<SaveLayoutMessage>()
    .add_message::<ResetLayoutMessage>()
    .add_message::<LoadLayoutMessage>()
    .add_systems(Startup, startup)
    .add_systems(
      FixedUpdate,
      (
        ResetLayoutMessage::handle,
        SaveLayoutMessage::handle,
        LoadLayoutMessage::handle,
      ),
    )
    .add_systems(
      EguiPrimaryContextPass,
      (save_layout_dialog_display, reset_layout_dialog_display).after(EditorUi),
    );
}

fn startup(mut settings: Settings, mut cached_settings: ResMut<CachedSettings>) {
  cached_settings.save_layout_on_exit = settings.get_or_default(SaveLayoutOnExitSetting);
  cached_settings.start_in_testing = settings.get_or_default(StartEditorInTestingSetting);
}

pub fn render(ui: &mut egui::Ui, mut params: Params<'_, '_>) {
  egui::MenuBar::new().ui(ui, |ui| {
    tools_menu(ui, &mut params);
    view_menu(ui, &mut params);
    game_control(ui, &mut params);
  });
}

fn tools_menu(ui: &mut egui::Ui, params: &mut Params) {
  ui.menu_button("Tools", |ui| {
    if ui.button("Spawn Empty Entity").clicked() {
      params.commands.spawn_empty();
    }

    if ui.button("Copy New UUID").clicked() {
      ui.output_mut(|output| {
        output
          .commands
          .push(egui::OutputCommand::CopyText(Uuid::new_v4().to_string()));
      });
    }
  });
}

fn view_menu(ui: &mut egui::Ui, params: &mut Params) {
  ui.menu_button("View", |ui| {
    layout_menu(ui, params);
    camera_menu(ui, params);
  });
}

fn game_control(ui: &mut egui::Ui, params: &mut Params) {
  match **params.editor_state {
    EditorState::Editing => {
      play_button(ui, params);
    }
    EditorState::Testing => {
      pause_button(ui, params);
    }
    _ => (),
  }

  ui.label("Start In Testing");
  if ui
    .checkbox(&mut params.cached_settings.start_in_testing, ())
    .clicked()
    && let Err(err) = params.settings.set(
      StartEditorInTestingSetting,
      params.cached_settings.start_in_testing,
    )
  {
    error!("{err}");
  }
  {}
}

fn layout_menu(ui: &mut egui::Ui, params: &mut Params) {
  ui.menu_button("Layouts", |ui| {
    ui.add_enabled_ui(!params.save_layout_dialog_state.open, |ui| {
      if ui.button("Save Layout").clicked() {
        params.save_layout_dialog_state.open = true;
      }
    });

    if !params.layout_manager.is_empty() {
      ui.add_enabled_ui(
        !params.save_layout_dialog_state.open && !params.reset_layout_dialog_state.open,
        |ui| {
          ui.menu_button("Restore", |ui| {
            for layout in params.layout_manager.iter() {
              if ui.button(layout).clicked() {
                params
                  .commands
                  .write_message(LoadLayoutMessage(layout.clone()));
              }
            }
          });
        },
      );
    }

    ui.add_enabled_ui(!params.reset_layout_dialog_state.open, |ui| {
      if ui.button("Restore Default").clicked() {
        params.reset_layout_dialog_state.open = true;
      }
    });

    ui.horizontal(|ui| {
      ui.label("Save On Exit");
      if ui
        .checkbox(&mut params.cached_settings.save_layout_on_exit, ())
        .clicked()
        && let Err(err) = params.settings.set(
          SaveLayoutOnExitSetting,
          params.cached_settings.save_layout_on_exit,
        )
      {
        error!("{err}");
      }
    });
  });
}

fn camera_menu(ui: &mut egui::Ui, params: &mut Params) {
  ui.menu_button("Camera", |ui| {
    if *params.editor_state == EditorState::Editing {
      camera_selector(ui, params);

      if *params.active_camera_state == ActiveEditorCamera::Cam3D {
        look_at_origin_button(ui, params);
      }

      entity_commands(ui, params);
    }
  });
}

fn camera_selector(ui: &mut egui::Ui, params: &mut Params) {
  for (text, state) in [
    ("Use 3D Camera", ActiveEditorCamera::Cam3D),
    ("Use 2D Camera", ActiveEditorCamera::Cam2D),
  ] {
    if ui.button(text).clicked() {
      params.next_active_camera.set(state);
    }
  }
}

fn look_at_origin_button(ui: &mut egui::Ui, params: &mut Params) {
  if ui.button("Look At Origin").clicked() {
    params.commands.trigger(PointCameraEvent::new(Vec3::ZERO));
  }
}

fn entity_commands(ui: &mut egui::Ui, params: &mut Params) {
  let InspectorSelection::Entities(selected_entities) = &*params.selection else {
    return;
  };

  let Some(entity) = (selected_entities.len() == 1)
    .then(|| selected_entities.iter().next())
    .flatten()
  else {
    return;
  };

  if matches!(
    **params.active_camera_state,
    ActiveEditorCamera::Cam2D | ActiveEditorCamera::Cam3D
  ) {
    move_to_target_button(ui, params, entity);

    if *params.active_camera_state == ActiveEditorCamera::Cam3D {
      look_at_target_button(ui, params, entity);
    }
  }
}

fn move_to_target_button(ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
  if ui.button("Move To Selected").clicked() {
    let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
      return;
    };

    params.commands.trigger(MoveCameraEvent::new(entity_pos));
  }
}

fn look_at_target_button(ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
  if ui.button("Look At Selected").clicked() {
    let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
      return;
    };

    params.commands.trigger(PointCameraEvent::new(entity_pos));
  }
}

fn play_button(ui: &mut egui::Ui, params: &mut Params) {
  if ui.button("▶").clicked() {
    params.next_editor_state.set(EditorState::Testing);
  }
}

fn pause_button(ui: &mut egui::Ui, params: &mut Params) {
  if ui.button("⏸").clicked() {
    params.next_editor_state.set(EditorState::Editing);
  }
}

fn save_layout_dialog_display(
  mut commands: Commands,
  mut state: ResMut<SaveLayoutDialogState>,
  mut ctx: Single<&mut bevy_egui::EguiContext>,
  mut layout_name: Local<String>,
) {
  if !state.open {
    layout_name.clear();
    return;
  }

  let mut open = state.open;
  components::Dialog::new("Save Layout").open(ctx.get_mut(), &mut open, |ui| {
    ui.horizontal(|ui| {
      ui.label("Name");
      ui.text_edit_singleline(&mut *layout_name);
    });

    ui.horizontal(|ui| {
      if ui.button("Save").clicked() {
        commands.write_message(SaveLayoutMessage(layout_name.take()));
      }

      if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::RED, error);
      }
    });
  });

  state.open = open;
}

fn reset_layout_dialog_display(
  mut commands: Commands,
  mut ctx: Single<&mut bevy_egui::EguiContext>,
  mut state: ResMut<ResetLayoutDialogState>,
) {
  if !state.open {
    return;
  }

  let mut open = state.open;

  components::Dialog::new("Confirm Layout Reset?").open(ctx.get_mut(), &mut open, |ui| {
    ui.label("This will reset your layout to the default configuration. Continue?");
    ui.horizontal(|ui| {
      if ui.button("Ok").clicked() {
        commands.write_message(ResetLayoutMessage);
      }
    });
  });

  state.open = open;
}

#[derive(Message)]
struct SaveLayoutMessage(String);

impl SaveLayoutMessage {
  fn handle(
    mut reader: MessageReader<Self>,
    mut state: ResMut<SaveLayoutDialogState>,
    ui_manager: Res<UiManager>,
    mut layout_manager: ResMut<LayoutManager>,
    q_uuids: Query<&PersistentId, Without<MissingUi>>,
    q_missing: Query<&MissingUi>,
    mut layouts: Layouts,
  ) {
    for msg in reader.read() {
      let dock = ui_manager
        .state()
        .decouple(&ui_manager, &q_uuids, &q_missing);
      if let Err(err) = layouts.save_layout(&msg.0, dock) {
        error!("{err}");
        state.error = Some(err.to_string());
      } else {
        layout_manager.insert(msg.0.clone());
        state.open = false;
      }
    }
  }
}

#[derive(Message)]
struct LoadLayoutMessage(String);

impl LoadLayoutMessage {
  fn handle(
    mut reader: MessageReader<Self>,
    mut commands: Commands,
    mut layouts: Layouts,
  ) -> Result {
    for msg in reader.read() {
      let layout_name = msg.0.clone();
      let dock = layouts.get_layout(layout_name)?;
      commands.queue(move |world: &mut World| {
        world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
          let new_state = DockState::restore(&dock, ui_manager.vtables(), world);
          ui_manager.switch_state(new_state, world);
        })
      });
    }

    Ok(())
  }
}

#[derive(Message)]
struct ResetLayoutMessage;

impl ResetLayoutMessage {
  fn handle(mut reader: MessageReader<ResetLayoutMessage>, mut commands: Commands) {
    if reader.is_empty() {
      return;
    }

    reader.clear();

    commands.queue(|world: &mut World| {
      world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
        let default_state = ui_manager.default_dock_state(world);
        ui_manager.switch_state(default_state, world);
        let mut state = world.resource_mut::<ResetLayoutDialogState>();
        state.open = false;
      });
    });
  }
}
