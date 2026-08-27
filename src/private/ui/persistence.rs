use super::{
	EditorEguiContext,
	misc::{DockExtensions as _, MissingUi},
};
use crate::{
	EditorState,
	private::{
		EditorInternalQuery, EditorInternalSingle,
		ui::{UiDockState, UiVTables},
		util::extensions::WorldMutExtensions as _,
	},
	storage::{
		DataTable, GlobalEditorSettings, PersistentData, ProjectSettings,
		settings::{CurrentLayoutSetting, EditorEguiSettings, EditorUiScale, SaveLayoutOnExitSetting},
	},
};
use bevy::prelude::*;
use bevy_egui::EguiContext;
use derive_new::new;
use egui_dock::DockState;
use persistent_id::PersistentId;
use serde::{Deserialize, Serialize};

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(
			OnEnter(EditorState::Exiting),
			(save_context_options, save_scale_factor, save_layouts),
		);
	}
}

pub struct LayoutsTable;

impl DataTable for LayoutsTable {
	type DataType = Vec<u8>;
	const TABLE: &str = "layouts";
	const KEY_COLUMN: &str = "name";
	const VALUE_COLUMN: &str = "data";
}

#[derive(new)]
pub struct SavedLayout(String);

impl PersistentData for SavedLayout {
	type Table = LayoutsTable;
	type Type = DockState<LayoutInfo>;

	fn to_key(self) -> String {
		self.0
	}

	fn serialize(value: Self::Type) -> Result<Vec<u8>> {
		let bytes = postcard::to_stdvec(&value)?;
		Ok(bytes)
	}

	fn deserialize(input: Vec<u8>) -> Result<Self::Type> {
		let value = postcard::from_bytes(&input)?;
		Ok(value)
	}
}

#[derive(Clone, Serialize, Deserialize, new)]
pub struct LayoutInfo {
	id: PersistentId,
	name: String,
}

impl LayoutInfo {
	pub fn id(&self) -> PersistentId {
		self.id
	}

	pub fn name(&self) -> &str {
		&self.name
	}
}

pub struct LoadLayout(pub DockState<LayoutInfo>);

impl Command for LoadLayout {
	type Out = ();
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, mut state: Mut<UiDockState>| {
			let new_state = DockState::restore(self.0, world);
			world
				.notify_on_error(
					|world| state.switch(new_state, world),
					|_, err| ("Failed to switch to layout", Some(err)),
				)
				.ok();
		})
	}
}

fn save_context_options(
	mut context: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	let ctx = context.get_mut();
	let opts = ctx.options(|opts| opts.clone());
	settings.set(EditorEguiSettings, opts).ok();
}

fn save_scale_factor(
	mut ctx: Single<&mut EguiContext, With<EditorEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	let ctx = ctx.get_mut();
	settings.set(EditorUiScale, ctx.zoom_factor()).ok();
}

fn save_layouts(
	state: Res<UiDockState>,
	vtables: Res<UiVTables>,
	q_uuids: EditorInternalQuery<&PersistentId, Without<MissingUi>>,
	q_missing: EditorInternalQuery<&MissingUi>,
	mut settings: ProjectSettings,
) -> Result {
	let save_on_exit = settings.get(SaveLayoutOnExitSetting).unwrap_or(true);
	let current_layout = if save_on_exit {
		let name = match settings.get(CurrentLayoutSetting).ok() {
			Some(opt) => opt,
			None => {
				let default_layout = String::from("default");
				settings.set(CurrentLayoutSetting, default_layout.clone())?;
				default_layout
			}
		};

		Some(name)
	} else {
		None
	};

	if let Some(name) = current_layout {
		let new_state = state.save(&vtables, &q_uuids, &q_missing);
		settings.set(SavedLayout(name), new_state)?;
	}

	Ok(())
}
