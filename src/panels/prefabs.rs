use std::{any::TypeId, borrow::Cow, num::NonZeroUsize};

use crate::{
	EditorOwned,
	panels::{BundleDnd, SearchableVfs, type_editor::OpenTypeEditor},
	ui::{EditorUiBundle, notifications::Notification, widgets::Card},
};
use bevy::prelude::*;
use brefabs::{Prefabs, SpawnUntypedPrefabEvent, WorldExtensions};
use uuid::{Uuid, uuid};
use vfs::Vfs;

#[derive(Component, Reflect, Default)]
#[require(EditorOwned)]
pub struct PrefabsUi;

impl EditorUiBundle for PrefabsUi {
	type PrimaryComponent = Self;

	const NAME: &str = stringify!(Prefabs);
	const ID: Uuid = uuid!("fa977fad-ed99-4842-bab4-7c00641b39b0");

	const UNIQUE: bool = true;

	fn init(app: &mut App) {
		app
			.init_resource::<PrefabVfsState>()
			.add_message::<EditPrefabDescriptorMessage>()
			.add_systems(
				FixedUpdate,
				(
					EditPrefabDescriptorMessage::handle,
					rebuild_vfs.run_if(resource_changed::<Prefabs>),
				),
			);
	}

	fn spawn(_entity: Entity, _world: &mut World) -> Self {
		default()
	}

	fn ui(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		world.resource_scope(|world, mut vfs_state: Mut<PrefabVfsState>| {
			let PrefabVfsState {
				vfs,
				searchable_vfs,
			} = vfs_state.as_mut();

			searchable_vfs.search_ui(ui, vfs);

			let num_columns = NonZeroUsize::new(20).unwrap();

			searchable_vfs.display_ui(ui, vfs, num_columns, |ui, size, basename, id, prefab| {
				ui_for_item(ui, size, basename, id, prefab, world);
			});
		});
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
struct PrefabVfsState {
	#[deref]
	vfs: Vfs<PrefabData>,
	searchable_vfs: SearchableVfs,
}

#[derive(Clone)]
struct PrefabData {
	type_id: TypeId,
	variant: Option<Name>,
}

fn rebuild_vfs(
	mut commands: Commands,
	mut prefab_vfs: ResMut<PrefabVfsState>,
	prefabs: Res<Prefabs>,
	app_type_regsitry: Res<AppTypeRegistry>,
) {
	info!("Rebuilding prefab VFS");

	let mut vfs = Vfs::default();

	let type_registry = app_type_regsitry.0.read();
	for (type_id, variants) in prefabs.iter() {
		let Some(type_registration) = type_registry.get(type_id) else {
			warn!(
				"Failed to get type registration for prefab type {type_id:?}. It will not be found in the editor."
			);
			continue;
		};

		for variant in variants.map(|(variant, _)| variant) {
			let type_name = type_registration.type_info().type_path_table().short_path();
			let Some(module_path) = type_registration
				.type_info()
				.type_path_table()
				.module_path()
			else {
				unreachable!("Every type should have a module path");
			};

			let name = match variant {
				Some(name) => Cow::Owned(format!("{type_name}#{name}")),
				None => Cow::Borrowed(type_name),
			};

			let Ok(path) = vfs.mkdir_p(module_path.split("::")).inspect_err(|err| {
				error!(type_name, err = err.to_string(), "Already registered");
			}) else {
				return;
			};

			if let Err(err) = vfs.new_item(
				path,
				name,
				PrefabData {
					type_id,
					variant: variant.clone(),
				},
			) {
				commands.trigger(Notification::error("Failed to add prefab").with_context(
					serde_json::json!({
						"module_path": module_path,
						"type_name": type_name,
						"err": err.to_string(),
					}),
				));
			}
		}
	}

	prefab_vfs.searchable_vfs.sync_current_node(&vfs);
	prefab_vfs.vfs = vfs;
}

fn ui_for_item(
	ui: &mut egui::Ui,
	size: egui::Vec2,
	label: &str,
	id: egui::Id,
	prefab_data: &PrefabData,
	world: &mut World,
) {
	let response = ui
		.dnd_drag_source(
			id,
			BundleDnd::AddPrefab(prefab_data.type_id, prefab_data.variant.clone()),
			|ui| {
				Card::new(size).with_label(label).show(ui, |ui| {
					ui.label(egui_phosphor_icons::icons::CUBE.regular());
				});
			},
		)
		.response;

	let response = ui.interact(response.rect, id, egui::Sense::click());

	let is_editable_prefab = world
		.resource::<Prefabs>()
		.meta(prefab_data.type_id, &prefab_data.variant)
		.is_some();

	response.context_menu(|ui| {
		if ui.button("Spawn").clicked() {
			world.trigger(SpawnUntypedPrefabEvent::new(
				prefab_data.type_id,
				prefab_data.variant.clone(),
			));
		}

		if is_editable_prefab && ui.button("Edit").clicked() {
			world.write_message(EditPrefabDescriptorMessage(
				prefab_data.type_id,
				prefab_data.variant.clone(),
			));
		}
	});
}

#[derive(Message)]
struct EditPrefabDescriptorMessage(TypeId, Option<Name>);

impl EditPrefabDescriptorMessage {
	fn handle(mut messages: MessageReader<Self>, mut commands: Commands) {
		for msg in messages.read() {
			let type_id = msg.0;
			let variant = msg.1.clone();

			commands.queue(move |world: &mut World| {
				if let Some(desc) = world.spawn_prefab_descriptor(type_id, variant) {
					world.commands().queue(OpenTypeEditor::new(desc));
				}
			});
		}
	}
}
