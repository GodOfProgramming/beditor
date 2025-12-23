use crate::{
	EditorUiBundle,
	ui::{TabState, UiManager, widgets},
	util::reflection::{ReflectDefaultCache, serde::SerdeRegistry},
};
use bevy::{prelude::*, reflect::TypeInfo};
use derive_new::new;
use egui_file_dialog::{DialogState, FileDialog};
use parking_lot::Mutex;
use std::{cell::RefCell, io::Write, path::PathBuf, sync::Arc};
use uuid::{Uuid, uuid};

#[derive(Bundle, Reflect, Default)]
pub struct TypeEditor {
	#[reflect(ignore)]
	state: TypeEditorState,
	_marker: TypeEditorMarker,
}

#[derive(Component, Reflect, Default)]
pub struct TypeEditorMarker;

impl EditorUiBundle for TypeEditor {
	type PrimaryComponent = TypeEditorMarker;

	const NAME: &str = stringify!(TypeEditor);

	const ID: Uuid = uuid!("2b01d041-d8b3-4cbe-8ca7-f6ae8e8ef7dd");

	const REOPEN_ON_STARTUP: bool = false;

	fn init(app: &mut App) {
		app
			.add_observer(on_editor_state_insert)
			.add_message::<SaveFileMessage>()
			.add_message::<OpenFileMessage>()
			.add_systems(
				FixedUpdate,
				(SaveFileMessage::handle, OpenFileMessage::handle),
			)
			.add_systems(bevy_egui::EguiPrimaryContextPass, show_dialogs);
	}

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		let mut entity_mut = world.entity_mut(entity);
		let Some(mut state) = entity_mut.get_mut::<TypeEditorState>() else {
			return;
		};

		let can_open_file_dialog = matches!(
			state.file_dialog.state(),
			DialogState::Closed | DialogState::Cancelled
		);

		let Some(arc) = state.value.as_ref().map(Arc::clone) else {
			if can_open_file_dialog {
				if ui.button("Open File").clicked() {
					state.file_dialog.pick_file();
				}

				if ui.button("Select...").clicked() {
					state.type_selection_dialog.open = true;
				}
			}

			ui.separator();

			return;
		};

		let m = arc.lock();
		let mut value = m.borrow_mut();

		let label = value.reflect_type_info().type_path();

		let mut message = None;

		ui.horizontal(|ui| {
			ui.heading(label);

			if can_open_file_dialog {
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					if ui.button("Save As").clicked() {
						state.file_dialog.save_file();
					}

					if let Some(opened_file) = &state.opened_file
						&& ui.button("Save").clicked()
					{
						message = Some(SaveFileMessage {
							entity,
							file: opened_file.clone(),
						});
					}
				});
			}
		});

		ui.separator();

		bevy_inspector_egui::bevy_inspector::ui_for_value(&mut **value, ui, world);

		if let Some(msg) = message {
			world.write_message(msg);
		}
	}
}

#[derive(Clone)]
struct CachedType {
	display: String,
	type_info: &'static TypeInfo,
}

impl CachedType {
	fn new(type_info: &'static TypeInfo) -> Self {
		Self {
			display: format!(
				"{} ({})",
				type_info.type_path_table().short_path(),
				type_info.type_path()
			),
			type_info,
		}
	}
}

impl PartialEq for CachedType {
	fn eq(&self, other: &Self) -> bool {
		self.type_info.type_id() == other.type_info.type_id()
	}
}

impl Eq for CachedType {}

impl AsRef<str> for CachedType {
	fn as_ref(&self) -> &str {
		&self.display
	}
}

#[derive(Component)]
struct TypeEditorState {
	opened_file: Option<PathBuf>,

	value: Option<Arc<Mutex<RefCell<Box<dyn Reflect>>>>>,

	file_dialog: FileDialog,

	type_selection_dialog: widgets::Dialog,
	type_list: widgets::SelectableList<CachedType>,
	type_filter: String,
	type_list_cache: Vec<CachedType>,
}

impl Default for TypeEditorState {
	fn default() -> Self {
		Self {
			opened_file: None,
			value: None,
			file_dialog: FileDialog::default(),
			type_selection_dialog: widgets::Dialog::new("Select Type"),
			type_list: default(),
			type_filter: default(),
			type_list_cache: default(),
		}
	}
}

impl TypeEditorState {
	fn new(value: Box<dyn Reflect>) -> Self {
		Self::default().with_value(value)
	}

	fn with_value(mut self, value: Box<dyn Reflect>) -> Self {
		self.set_value(value);
		self
	}

	fn set_value(&mut self, value: Box<dyn Reflect>) {
		self.value = Some(Arc::new(Mutex::new(RefCell::new(value))));
	}
}

#[derive(new)]
pub struct OpenTypeEditor(Box<dyn Reflect>);

impl Command for OpenTypeEditor {
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
			let tab = TabState::spawn::<TypeEditor>(world);
			world
				.entity_mut(tab.entity)
				.insert(TypeEditorState::new(self.0));
			ui_manager.add_tab_to_focused(tab);
		});
	}
}

fn on_editor_state_insert(
	event: On<Add, TypeEditorState>,
	mut q_states: Query<&mut TypeEditorState>,
	cache: Res<ReflectDefaultCache>,
) {
	let Ok(mut state) = q_states.get_mut(event.event_target()) else {
		return;
	};

	state.type_list_cache = cache
		.iter()
		.map(|type_info| CachedType::new(type_info))
		.collect();
}

fn show_dialogs(
	mut commands: Commands,
	mut q_states: Query<(Entity, &mut TypeEditorState)>,
	mut contexts: bevy_egui::EguiContexts,
	cache: Res<ReflectDefaultCache>,
	app_type_registry: Res<AppTypeRegistry>,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	for (entity, mut state) in &mut q_states {
		let TypeEditorState {
			ref mut file_dialog,
			ref mut type_selection_dialog,
			ref mut type_list,
			ref mut type_filter,
			ref mut type_list_cache,
			..
		} = *state;

		file_dialog.update(ctx);
		if let Some(file) = file_dialog.take_picked() {
			match file_dialog.mode() {
				egui_file_dialog::DialogMode::PickFile => {
					commands.write_message(OpenFileMessage::new(entity, file.to_path_buf()));
				}
				egui_file_dialog::DialogMode::SaveFile => {
					commands.write_message(SaveFileMessage::new(entity, file.to_path_buf()));
				}
				_ => (),
			}
		}

		let response = type_selection_dialog.show(ctx, |ui, open| {
			if ui.text_edit_singleline(type_filter).changed() || cache.is_changed() {
				let filter = type_filter.to_lowercase();

				*type_list_cache = cache
					.iter()
					.filter_map(|type_info| {
						let full_path = type_info.type_path();
						full_path
							.contains(&filter)
							.then(|| CachedType::new(type_info))
					})
					.collect();
			}

			if let Some(inner_response) = type_list.ui(ui, type_list_cache) {
				let Some(type_info) = type_list.selected().and_then(|selected| {
					cache
						.iter()
						.find(|t| selected.type_info.type_id() == t.type_id())
				}) else {
					warn!("Logic error indexing default cache");
					return None;
				};

				let type_registry = app_type_registry.read();

				let Some(type_registration) = type_registry.get(type_info.type_id()) else {
					warn!("Logic error indexing a type id that previously existed");
					return None;
				};

				let Some(reflect_default) = type_registration.data::<ReflectDefault>() else {
					warn!("Logic error accessing reflect default for a type that had reflect default");
					return None;
				};

				if inner_response.response.clicked() {
					*open = false;
				}

				Some(reflect_default.default())
			} else {
				None
			}
		});

		// this might be the dumbest thing I ever wrote
		if let Some(response) = response
			&& let Some(Some(value)) = response.inner
		{
			state.set_value(value);
		}
	}
}

#[derive(new, Message)]
struct OpenFileMessage {
	entity: Entity,
	file: PathBuf,
}

impl OpenFileMessage {
	fn handle(
		mut messages: MessageReader<Self>,
		mut q_states: Query<&mut TypeEditorState>,
		loaders: Res<SerdeRegistry>,
		app_type_registry: Res<AppTypeRegistry>,
	) -> Result {
		for msg in messages.read() {
			let Some(de) = loaders.deserializer_for(&msg.file) else {
				warn!(
					path = msg.file.display().to_string(),
					"No deserializer registered for file type"
				);
				continue;
			};

			let Ok(mut state) = q_states.get_mut(msg.entity) else {
				warn!(
					entity = msg.entity.to_string(),
					"Failed to get type editor state for entity"
				);

				continue;
			};

			let type_registry = app_type_registry.read();

			let bytes = std::fs::read(&msg.file)?;

			let value = (de)(&bytes, &type_registry)?;

			state.opened_file = Some(msg.file.clone());

			state.set_value(value);
		}

		Ok(())
	}
}

#[derive(new, Message)]
struct SaveFileMessage {
	entity: Entity,
	file: PathBuf,
}

impl SaveFileMessage {
	fn handle(
		mut messages: MessageReader<Self>,
		mut q_states: Query<&mut TypeEditorState>,
		registry: Res<SerdeRegistry>,
		app_type_registry: Res<AppTypeRegistry>,
	) -> Result {
		for msg in messages.read() {
			let Some(ser) = registry.serializer_for(&msg.file) else {
				warn!(
					path = msg.file.display().to_string(),
					"No loader registered for file type"
				);
				continue;
			};

			let Ok(mut state) = q_states.get_mut(msg.entity) else {
				warn!(
					entity = msg.entity.to_string(),
					"Failed to get type editor state for entity"
				);

				continue;
			};

			state.opened_file = Some(msg.file.clone());

			let Some(value) = &state.value else {
				warn!("Tried to save None value");
				continue;
			};

			let type_registry = app_type_registry.read();

			let value = value.lock();
			let value = value.borrow();
			let value = &**value;

			let bytes = (ser)(value, &type_registry)?;
			let path = msg.file.clone();

			let mut file = std::fs::OpenOptions::new()
				.write(true)
				.create(true)
				.truncate(true)
				.open(path)?;

			file.write_all(&bytes)?;
		}

		Ok(())
	}
}
