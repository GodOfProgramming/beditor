use super::{ContentDef, ContentDefAsset, ContentDefs};
use crate::private::{
	EditorInternal, EditorInternalQuery, EditorInternalSingle, ui::EditorEguiContext,
};
use async_walkdir::Filtering;
use bevy::{
	asset::{AssetLoader, AssetPath, LoadedFolder},
	platform::{collections::HashMap, sync::atomic},
	prelude::*,
	tasks::{
		IoTaskPool, Task,
		futures::check_ready,
		futures_lite::{StreamExt, stream},
	},
};
use bevy_egui::EguiContext;
use derive_new::new;
use egui::Widget;
use inflector::Inflector;
use std::{
	ffi::OsStr,
	path::PathBuf,
	sync::{Arc, atomic::Ordering},
};
use tokio::sync::watch;

#[derive(Resource)]
pub struct ContentDefTotal(usize);

#[derive(new, Resource, Deref, DerefMut)]
pub struct ContentDefProgress(watch::Receiver<(usize, Option<PathBuf>)>);

#[derive(Component)]
#[require(EditorInternal)]
pub struct FileCountTask(Task<usize>);

#[derive(Resource, Deref)]
pub struct ContentFolder(Handle<LoadedFolder>);

#[derive(Deref, DerefMut)]
pub struct AssetLoadModal(widgets::MenuModal);

impl Default for AssetLoadModal {
	fn default() -> Self {
		Self(
			widgets::MenuModal::new()
				.closeable(false)
				.with_proportion_to_window(0.3),
		)
	}
}

#[derive(Deref)]
pub struct AssetLoadModalId(egui::Id);

impl Default for AssetLoadModalId {
	fn default() -> Self {
		Self(egui::Id::new("beditor-editor-asset-load-modal"))
	}
}

pub fn load_content(mut commands: Commands, asset_server: Res<AssetServer>) {
	let loaded_folder_handle = asset_server.load_folder("content");
	commands.insert_resource(ContentFolder(loaded_folder_handle));

	let io_task_pool = IoTaskPool::get();
	let task = io_task_pool.spawn(
		async_walkdir::WalkDir::new("assets/content")
			.filter(async |ent| {
				if ent.path().extension() == Some(OsStr::new("ron")) {
					Filtering::Continue
				} else {
					Filtering::Ignore
				}
			})
			.flat_map(stream::iter)
			.count(),
	);

	commands.spawn(FileCountTask(task));
}

pub fn display_load_progress(
	mut contexts: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
	total: Option<Res<ContentDefTotal>>,
	progress: Res<ContentDefProgress>,
	mut modal: Local<AssetLoadModal>,
	mut asset_events: MessageReader<AssetEvent<LoadedFolder>>,
	id: Local<AssetLoadModalId>,
	loaded_folder_handle: Res<ContentFolder>,
	mut loading_finished: Local<bool>,
) {
	*loading_finished = *loading_finished
		|| asset_events.read().any(|msg| {
			if let AssetEvent::LoadedWithDependencies { id } = msg {
				*id == loaded_folder_handle.id()
			} else {
				false
			}
		});

	if *loading_finished {
		return;
	}

	let Some(total) = total else {
		return;
	};

	let total = total.0;
	let (current, path) = &*progress.borrow();

	let is_counting = *current != 0;

	if is_counting && let Some(path) = path {
		modal.open = true;

		let ctx = contexts.get_mut();
		modal.show(ctx, **id, |ui| {
			ui.label("TODO help text or something fun");

			ui.scope_builder(
				egui::UiBuilder::new().layout(egui::Layout::bottom_up(egui::Align::Min)),
				|ui| {
					egui::ProgressBar::new(*current as f32 / total as f32)
						.text(format!("{current} / {total}"))
						.ui(ui);

					ui.label(format!("Loading {}", path.display()));
				},
			);
		});
	}
}

#[derive(Default, TypePath)]
pub struct ContentDefLoader {
	count: atomic::AtomicUsize,
	output: watch::Sender<(usize, Option<PathBuf>)>,
}

impl ContentDefLoader {
	pub fn new(output: watch::Sender<(usize, Option<PathBuf>)>) -> Self {
		Self {
			count: default(),
			output,
		}
	}
}

impl AssetLoader for ContentDefLoader {
	type Asset = ContentDefAsset;

	type Settings = ();

	type Error = BevyError;

	async fn load(
		&self,
		reader: &mut dyn bevy::asset::io::Reader,
		_: &Self::Settings,
		load_context: &mut bevy::asset::LoadContext<'_>,
	) -> Result<Self::Asset, Self::Error> {
		let mut buf = Vec::new();
		reader.read_to_end(&mut buf).await?;
		let content_def = ron::de::from_bytes::<Arc<dyn ContentDef>>(&buf)?;

		let prev = self.count.fetch_add(1, Ordering::Relaxed);
		let asset_path = load_context.path();
		self
			.output
			.send((prev + 1, Some(asset_path.path().to_path_buf())))?;

		Ok(ContentDefAsset::new(content_def, asset_path.clone()))
	}

	fn extensions(&self) -> &[&str] {
		&["ron"]
	}
}

pub fn poll_file_count_task(
	mut commands: Commands,
	mut q_tasks: EditorInternalQuery<(Entity, &mut FileCountTask)>,
) {
	for (entity, mut task) in &mut q_tasks {
		let Some(file_count) = check_ready(&mut task.0) else {
			continue;
		};

		commands.insert_resource(ContentDefTotal(file_count));
		commands.entity(entity).despawn();
	}
}

pub fn poll_content_loading(
	mut content_defs: ResMut<ContentDefs>,
	mut asset_events: MessageReader<AssetEvent<ContentDefAsset>>,
	mut content_def_assets: ResMut<Assets<ContentDefAsset>>,
	mut handle_node_map: Local<HashMap<AssetId<ContentDefAsset>, vfs::VfsNode>>,
) {
	for msg in asset_events.read() {
		match msg {
			AssetEvent::Added { id }
			| AssetEvent::Modified { id }
			| AssetEvent::LoadedWithDependencies { id } => {
				let Some(handle) = content_def_assets.get_strong_handle(*id) else {
					continue;
				};

				let Some(content_def) = content_def_assets.get(*id) else {
					error!("No content def for asset id");
					continue;
				};

				let Some((dir_path, name)) = vfs_entry_from_path(&content_def.asset_path) else {
					continue;
				};

				let Ok(dir_node) = content_defs.mkdir_p(dir_path) else {
					error!("Could not make content def dir in vfs");
					continue;
				};

				let humanized_name = name.to_string_lossy().to_sentence_case();

				let mut count = 0;
				'add_item: loop {
					match content_defs.new_item(dir_node, humanized_name.clone(), handle.clone()) {
						Ok(node) => {
							handle_node_map.insert(*id, node);
							break 'add_item;
						}
						Err(err) => match err {
							vfs::VfsError::ItemAlreadyExists(existing_item) => {
								content_defs.rm(existing_item);

								handle_node_map.remove(id);

								count += 1;
								debug_assert!(count < 2);
							}
							err => {
								error!("Failed to register asset {err}");
								break 'add_item;
							}
						},
					}
				}
			}
			AssetEvent::Removed { id } => {
				let Some(node) = handle_node_map.remove(id) else {
					error!("Removed asset that was not added");
					continue;
				};
				content_defs.rm(node);
			}
			AssetEvent::Unused { id: _ } => {}
		}
	}
}

fn vfs_entry_from_path<'a>(
	asset_path: &'a AssetPath<'static>,
) -> Option<(impl Iterator<Item = std::borrow::Cow<'a, str>>, &'a OsStr)> {
	let path = asset_path.path();

	let Some(parent) = path.parent() else {
		error!("Asset path has no parent: {}", path.display());
		return None;
	};

	let dir_path = parent.components().map(|c| c.as_os_str().to_string_lossy());

	let Some(name) = path.file_stem() else {
		error!("Asset path has no file stem: {}", path.display());
		return None;
	};

	Some((dir_path, name))
}
