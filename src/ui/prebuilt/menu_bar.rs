use crate::{
  EditorState, Ui, UiManager,
  misc::{DockExtensions, MissingUi},
  ui::{EditorUi, InspectorSelection, components, managers::LayoutManager},
  view::{ActiveEditorCamera, MoveCameraEvent, PointCameraEvent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use egui::TextBuffer;
use egui_dock::DockState;
use persistent_id::PersistentId;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct MenuBar;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  commands: Commands<'w, 's>,

  editor_state: Res<'w, State<EditorState>>,
  next_editor_state: ResMut<'w, NextState<EditorState>>,
  active_camera_state: Res<'w, State<ActiveEditorCamera>>,
  next_active_camera: ResMut<'w, NextState<ActiveEditorCamera>>,
  selection: Res<'w, InspectorSelection>,

  layout_manager: Res<'w, LayoutManager>,

  load_layout_ew: EventWriter<'w, LoadLayoutEvent>,
  save_layout_ew: EventWriter<'w, SaveLayoutEvent>,
  reset_layout_ew: EventWriter<'w, ResetLayoutEvent>,
  move_camera_ew: EventWriter<'w, MoveCameraEvent>,
  point_camera_ew: EventWriter<'w, PointCameraEvent>,

  q_transforms: Query<'w, 's, &'static Transform>,
}

impl Ui for MenuBar {
  const NAME: &str = "Main Menu";

  const ID: Uuid = uuid!("e32d6fa9-2735-4ddf-9a26-b3fbfdb921e3");

  type Params<'w, 's> = Params<'w, 's>;

  fn init(app: &mut App) {
    app
      .add_event::<SaveLayoutEvent>()
      .add_event::<LoadLayoutEvent>()
      .add_event::<ResetLayoutEvent>()
      .add_systems(
        Update,
        (
          SaveLayoutEvent::handle,
          LoadLayoutEvent::handle,
          ResetLayoutEvent::handle,
        )
          .after(EditorUi),
      );
  }

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    Self
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    egui::MenuBar::new().ui(ui, |ui| {
      self.tools_menu(ui, &mut params);
      self.view_menu(ui, &mut params);
      self.game_control(ui, &mut params);
    });
  }

  fn unique() -> bool {
    true
  }
}

impl MenuBar {
  fn tools_menu(&self, ui: &mut egui::Ui, params: &mut Params) {
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

  fn view_menu(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.menu_button("View", |ui| {
      self.layout_menu(ui, params);
      self.camera_menu(ui, params);
    });
  }

  fn game_control(&self, ui: &mut egui::Ui, params: &mut Params) {
    match **params.editor_state {
      EditorState::Editing => {
        self.play_button(ui, params);
      }
      EditorState::Testing => {
        self.pause_button(ui, params);
      }
      _ => (),
    }
  }

  fn layout_menu(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.menu_button("Layouts", |ui| {
      if ui.button("Save Layout").clicked() {
        params.save_layout_ew.write(SaveLayoutEvent);
      }

      if !params.layout_manager.is_empty() {
        ui.menu_button("Restore", |ui| {
          for layout in params.layout_manager.keys() {
            if ui.button(layout).clicked() {
              params.load_layout_ew.write(LoadLayoutEvent(layout.clone()));
            }
          }
        });
      }

      if ui.button("Restore Default").clicked() {
        params.reset_layout_ew.write(ResetLayoutEvent);
      }
    });
  }

  fn camera_menu(&self, ui: &mut egui::Ui, params: &mut Params) {
    ui.menu_button("Camera", |ui| {
      if *params.editor_state == EditorState::Editing {
        self.camera_selector(ui, params);

        if *params.active_camera_state == ActiveEditorCamera::Cam3D {
          self.look_at_origin_button(ui, params);
        }

        self.entity_commands(ui, params);
      }
    });
  }

  fn camera_selector(&self, ui: &mut egui::Ui, params: &mut Params) {
    for (text, state) in [
      ("Use 3D Camera", ActiveEditorCamera::Cam3D),
      ("Use 2D Camera", ActiveEditorCamera::Cam2D),
    ] {
      if ui.button(text).clicked() {
        params.next_active_camera.set(state);
      }
    }
  }

  fn look_at_origin_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("Look At Origin").clicked() {
      params
        .point_camera_ew
        .write(PointCameraEvent::new(Vec3::ZERO));
    }
  }

  fn entity_commands(&self, ui: &mut egui::Ui, params: &mut Params) {
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
      self.move_to_target_button(ui, params, entity);

      if *params.active_camera_state == ActiveEditorCamera::Cam3D {
        self.look_at_target_button(ui, params, entity);
      }
    }
  }

  fn move_to_target_button(&self, ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
    if ui.button("Move To Selected").clicked() {
      let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
        return;
      };

      params
        .move_camera_ew
        .write(MoveCameraEvent::new(entity_pos));
    }
  }

  fn look_at_target_button(&self, ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
    if ui.button("Look At Selected").clicked() {
      let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
        return;
      };

      params
        .point_camera_ew
        .write(PointCameraEvent::new(entity_pos));
    }
  }

  fn play_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("▶").clicked() {
      params.next_editor_state.set(EditorState::Testing);
    }
  }

  fn pause_button(&self, ui: &mut egui::Ui, params: &mut Params) {
    if ui.button("⏸").clicked() {
      params.next_editor_state.set(EditorState::Editing);
    }
  }
}

#[derive(Event)]
struct SaveLayoutEvent;

impl SaveLayoutEvent {
  fn handle(
    events: EventReader<Self>,
    mut ctx: Single<&mut bevy_egui::EguiContext>,
    mut should_show: Local<bool>,
    mut save_name_text: Local<String>,
    ui_manager: Option<Res<UiManager>>,
    mut layout_manager: ResMut<LayoutManager>,
    q_uuids: Query<&PersistentId, Without<MissingUi>>,
    q_missing: Query<&MissingUi>,
  ) {
    if !events.is_empty() {
      *should_show = true;
      save_name_text.clear();
    }

    if let Some(ui_manager) = &ui_manager {
      *should_show = Self::show_dialog(
        ctx.get_mut(),
        *should_show,
        &mut save_name_text,
        ui_manager,
        &mut layout_manager,
        &q_uuids,
        &q_missing,
      );
    }
  }

  fn show_dialog(
    ctx: &egui::Context,
    should_show: bool,
    save_name_text: &mut String,
    ui_manager: &UiManager,
    layout_manager: &mut LayoutManager,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> bool {
    let mut save_clicked = false;

    let open = components::Dialog::new("Save Layout").open(ctx, should_show, |ui| {
      ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(save_name_text);
      });
      ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
          let dock = ui_manager.state().decouple(ui_manager, q_uuids, q_missing);
          layout_manager.insert(save_name_text.take(), dock);
          save_clicked = true;
        }
      });
    });

    !save_clicked && open
  }
}

#[derive(Event)]
struct LoadLayoutEvent(String);

impl LoadLayoutEvent {
  fn handle(
    mut commands: Commands,
    mut events: EventReader<Self>,
    layout_manager: Res<LayoutManager>,
  ) {
    for event in events.read() {
      let layout_name = event.0.clone();
      let dock = layout_manager[&layout_name].clone();
      commands.queue(move |world: &mut World| {
        world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
          let new_state = DockState::restore(&dock, ui_manager.vtables(), world);
          ui_manager.switch_state(new_state, world);
        })
      });
    }
  }
}

#[derive(Event)]
struct ResetLayoutEvent;

impl ResetLayoutEvent {
  fn handle(
    mut commands: Commands,
    events: EventReader<Self>,
    mut ctx: Single<&mut bevy_egui::EguiContext>,
    mut should_show: Local<bool>,
  ) {
    if !events.is_empty() {
      *should_show = true;
    }

    *should_show = Self::show_dialog(&mut commands, ctx.get_mut(), *should_show);
  }

  fn show_dialog(commands: &mut Commands, ctx: &egui::Context, should_show: bool) -> bool {
    let mut ok_clicked = false;

    let open = components::Dialog::new("Confirm Layout Reset?").open(ctx, should_show, |ui| {
      ui.label("This will reset your layout to the default configuration. Continue?");
      ui.horizontal(|ui| {
        if ui.button("Ok").clicked() {
          commands.queue(|world: &mut World| {
            world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
              let default_state = ui_manager.default_dock_state(world);
              ui_manager.switch_state(default_state, world);
            });
          });
          ok_clicked = true;
        }
      });
    });

    !ok_clicked && open
  }
}
