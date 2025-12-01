use std::{any::TypeId, borrow::Cow};

use crate::{
	BundleDnd,
	ui::{
		EditorUiBundle,
		builtin::type_editor::OpenTypeEditor,
		notifications::Notification,
		widgets::{Card, horizontal_list},
	},
	util::vfs::{Vfs, VfsNode, VfsPath},
};
use bevy::prelude::*;
use brefabs::{Prefabs, SpawnUntypedPrefabEvent, WorldExtensions};
use uuid::{Uuid, uuid};

#[derive(Component, Reflect, Default)]
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

	fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		world.resource_scope(|world, mut vfs_state: Mut<PrefabVfsState>| {
			let PrefabVfsState {
				vfs,
				current_path,
				filter,
			} = vfs_state.as_mut();

			let current_path = current_path.get_or_insert_with(|| vfs.root().clone());

			ui.horizontal(|ui| {
				ui.text_edit_singleline(&mut *filter);

				if current_path.has_parent(vfs)
					&& ui
						.button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
						.clicked()
					&& let Some(parent) = current_path.parent(vfs)
				{
					*current_path = parent.clone();
				};

				if egui::DragAndDrop::has_payload_of_type::<BundleDnd>(ui.ctx()) {
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
						super::dnd_prep_ui(ui);
						let (_, dnd) = ui.dnd_drop_zone::<BundleDnd, ()>(egui::Frame::new(), |ui| {
							let stroke =
								egui::Stroke::new(2.0, ui.style().visuals.widgets.active.fg_stroke.color);
							egui::Frame::default().stroke(stroke).show(ui, |ui| {
								ui.label("Create New");
							});
						});

						if let Some(bundle) = dnd {
							let entity = world.spawn_empty().id();
							bundle.spawn_on(std::iter::once(entity), world);
						}
					});
				}
			});

			ui.label(current_path.display());

			let prefabs = vfs.iter(current_path).filter(|path| {
				filter.is_empty() || {
					path
						.basename()
						.to_lowercase()
						.contains(filter.to_lowercase().as_str())
				}
			});

			let mut next_path = None;
			let num_columns = 20;

			horizontal_list(ui, num_columns, prefabs, |ui, i, path| {
				let card_width = ui.available_width();
				let card_height = card_width;

				let Some(node) = vfs.read(path) else {
					return;
				};

				match node {
					VfsNode::Dir => {
						if ui_for_dir(ui, (card_width, card_height), path.basename(), i) {
							next_path = Some(path.clone());
						}
					}
					VfsNode::Item { value } => {
						ui_for_item(ui, (card_width, card_height), path, value, world);
					}
				}
			});

			if let Some(path) = next_path {
				*current_path = path;
			}
		});
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
struct PrefabVfsState {
	#[deref]
	vfs: Vfs<PrefabData>,
	current_path: Option<VfsPath>,
	filter: String,
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
			warn!("Failed to get type registration for prefab. It will not be found in the editor.");
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

			let Some(path) = vfs.mkdir_p(module_path.split("::")) else {
				error!(type_name, "Already registered");
				return;
			};

			if let Err(path) = vfs.new_item(
				path,
				Name::new(name),
				PrefabData {
					type_id,
					variant: variant.clone(),
				},
			) {
				commands.trigger(Notification::error("Failed to add prefab").with_context(
					serde_json::json!({
						"module_path": module_path,
						"type_name": type_name,
						"path": path.full_path(),
					}),
				));
			}
		}
	}

	prefab_vfs.current_path = prefab_vfs
		.current_path
		.as_ref()
		.and_then(|path| vfs.find(path.full_path()))
		.cloned();

	prefab_vfs.vfs = vfs;
}

fn ui_for_dir(ui: &mut egui::Ui, size: impl Into<egui::Vec2>, label: &str, i: usize) -> bool {
	let size = size.into();
	Card::new(size)
		.with_label(label)
		.show(ui, |ui| {
			ui.label(egui_phosphor_icons::icons::FOLDER.regular());

			ui.interact(ui.min_rect(), ui.id().with(i), egui::Sense::click())
		})
		.inner
		.on_hover_cursor(egui::CursorIcon::PointingHand)
		.double_clicked()
}

fn ui_for_item(
	ui: &mut egui::Ui,
	size: impl Into<egui::Vec2>,
	path: &VfsPath,
	prefab_data: &PrefabData,
	world: &mut World,
) {
	let size = size.into();

	let id = egui::Id::new(path.display());

	let response = ui
		.dnd_drag_source(
			id,
			BundleDnd::AddPrefab(prefab_data.type_id, prefab_data.variant.clone()),
			|ui| {
				Card::new(size).with_label(path.basename()).show(ui, |ui| {
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
