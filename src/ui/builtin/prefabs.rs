use std::{any::TypeId, borrow::Cow};

use crate::{
	BundleDnd,
	ui::{
		EditorUiBundle,
		builtin::type_editor::OpenTypeEditor,
		notifications::Notification,
		widgets::{Card, horizontal_list},
	},
};
use bevy::{platform::collections::HashMap, prelude::*};
use brefabs::{Prefabs, SpawnUntypedPrefabEvent, WorldExtensions};
use uuid::{Uuid, uuid};
use vfs::{Vfs, VfsEntry, VfsNode};

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
				current_node,
				current_node_display,
				filter,
				id_cache,
			} = vfs_state.as_mut();

			let current_node = current_node.get_or_insert_with(|| {
				let root = vfs.root();
				*current_node_display = root.absolute(vfs).expect("root must exist");
				root
			});

			ui.horizontal(|ui| {
				ui.text_edit_singleline(&mut *filter);

				if current_node.has_parent(vfs)
					&& ui
						.button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
						.clicked()
					&& let Some(parent) = current_node.parent(vfs)
				{
					*current_node = parent;
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

			ui.label(&*current_node_display);

			let prefabs = vfs.ls(*current_node).filter(|node| {
				filter.is_empty() || {
					node
						.basename(vfs)
						.map(|name| name.to_lowercase().contains(filter.to_lowercase().as_str()))
						.unwrap_or(false)
				}
			});

			let mut next_path = None;
			let num_columns = 20;

			horizontal_list(ui, num_columns, prefabs, |ui, i, node| {
				let card_width = ui.available_width();
				let card_height = card_width;

				let Some(entry) = vfs.read(node) else {
					return;
				};

				let Some(basename) = node.basename(vfs) else {
					return;
				};

				let id = id_cache
					.entry(node)
					.or_insert_with(|| egui::Id::new(node.absolute(vfs)));

				match entry {
					VfsEntry::Dir => {
						if ui_for_dir(ui, (card_width, card_height), basename, i) {
							next_path = Some(node);
						}
					}
					VfsEntry::Item { value } => {
						ui_for_item(ui, (card_width, card_height), *id, basename, value, world);
					}
				}
			});

			if let Some(node) = next_path
				&& let Some(abs_path) = node.absolute(vfs)
			{
				*current_node = node;
				*current_node_display = abs_path;
			}
		});
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
struct PrefabVfsState {
	#[deref]
	vfs: Vfs<PrefabData>,
	current_node: Option<VfsNode>,
	current_node_display: String,
	filter: String,
	id_cache: HashMap<VfsNode, egui::Id>,
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

	prefab_vfs.current_node = prefab_vfs.current_node.as_ref().and_then(|node| {
		node
			.absolute(&prefab_vfs.vfs)
			.and_then(|ap| vfs.find_absolute(ap))
	});

	prefab_vfs.vfs = vfs;
	prefab_vfs.id_cache.clear();
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
	id: egui::Id,
	name: &str,
	prefab_data: &PrefabData,
	world: &mut World,
) {
	let size = size.into();

	let response = ui
		.dnd_drag_source(
			id,
			BundleDnd::AddPrefab(prefab_data.type_id, prefab_data.variant.clone()),
			|ui| {
				Card::new(size).with_label(name).show(ui, |ui| {
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
