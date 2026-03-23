use async_std::fs;
use async_walkdir::{DirEntry, Filtering};
use bevy::{
	asset::AssetPath,
	prelude::*,
	tasks::{
		IoTaskPool, Task,
		futures::check_ready,
		futures_lite::{StreamExt, stream},
	},
};
use bevy_egui::EguiContext;
use egui::Widget;
use inflector::Inflector;
use macros::EditorAsset;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::PathBuf, sync::Arc};
use struple::Struple;
use tokio::sync::watch;

use crate::private::{
	EditorInternal, EditorInternalQuery, EditorInternalSingle,
	ui::{EditorEguiContext, EditorUiEguiContextPass},
};

#[derive(Default)]
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<ContentDefs>()
			.add_systems(Startup, load_all_content)
			.add_systems(FixedUpdate, (poll_load_prep_task, poll_content_loading))
			.add_systems(EditorUiEguiContextPass, display_load_progress);
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ContentDefs(vfs::Vfs<Arc<dyn ContentDef>>);

#[typetag::serde(tag = "ns", content = "data")]
pub trait ContentDef: 'static + Send + Sync + ContentHandlers {}

pub trait ContentHandlers {
	fn insert_into_entities(&self, entity: Entity, world: &mut World);
}

#[derive(Reflect, Serialize, Deserialize, EditorAsset)]
#[ns("editor")]
pub enum EditorAssetDefs {
	Audio { source: PathBuf },
}

impl ContentHandlers for EditorAssetDefs {
	fn insert_into_entities(&self, entity: Entity, world: &mut World) {
		match self {
			EditorAssetDefs::Audio { source } => {
				let source_handle = world.load_asset(AssetPath::from_path(source));
				world
					.entity_mut(entity)
					.insert(AudioPlayer::new(source_handle));
			}
		}
	}
}

#[derive(Component)]
#[require(EditorInternal)]
struct LoadPrepTask(
	Task<(
		usize,
		watch::Receiver<(usize, Option<PathBuf>)>,
		Task<Vec<ContentInfo>>,
	)>,
);

#[derive(Component, Struple)]
#[require(EditorInternal)]
struct ContentLoadProgress(usize, watch::Receiver<(usize, Option<PathBuf>)>);

#[derive(Component)]
#[require(EditorInternal)]
struct ContentLoadTask(Task<Vec<ContentInfo>>);

#[derive(Struple)]
struct ContentInfo {
	file: PathBuf,
	def: Arc<dyn ContentDef>,
}

fn load_all_content(mut commands: Commands) {
	let io_task_pool = IoTaskPool::get();
	let task = io_task_pool.spawn(async {
		let files_to_load = async_walkdir::WalkDir::new("assets")
			.filter(async |ent| {
				if ent.path().extension() == Some(OsStr::new("bass")) {
					Filtering::Continue
				} else {
					Filtering::Ignore
				}
			})
			.flat_map(stream::iter)
			.collect::<Vec<_>>()
			.await;

		let file_count = files_to_load.len();

		let (tx, rx) = watch::channel((0, None));

		let io_task_pool = IoTaskPool::get();
		let asset_load_task = io_task_pool.spawn(load_asset_defs(files_to_load, tx));

		(file_count, rx, asset_load_task)
	});

	commands.spawn(LoadPrepTask(task));
}

fn poll_load_prep_task(
	mut commands: Commands,
	mut q_tasks: EditorInternalQuery<(Entity, &mut LoadPrepTask)>,
) {
	for (entity, mut task) in &mut q_tasks {
		let Some((file_count, progress_output, load_task)) = check_ready(&mut task.0) else {
			continue;
		};

		commands.spawn(ContentLoadProgress(file_count, progress_output));
		commands.spawn(ContentLoadTask(load_task));
		commands.entity(entity).despawn();
	}
}

fn poll_content_loading(
	mut commands: Commands,
	mut q_tasks: EditorInternalQuery<(Entity, &mut ContentLoadTask)>,
	q_progress: EditorInternalQuery<Entity, With<ContentLoadProgress>>,
	mut content_defs: ResMut<ContentDefs>,
) {
	for (entity, mut task) in &mut q_tasks {
		let Some(content_infos) = check_ready(&mut task.0) else {
			continue;
		};

		for entity in std::iter::once(entity).chain(q_progress.iter()) {
			commands.entity(entity).despawn();
		}

		for info in content_infos {
			let Some(parent) = info.file.parent() else {
				unreachable!();
			};

			let dir_path = parent.components().map(|c| c.as_os_str().to_string_lossy());
			let Ok(path) = content_defs.mkdir_p(dir_path) else {
				error!("Failed to register asset {}", info.file.display());
				continue;
			};

			let Some(name) = info.file.file_stem() else {
				unreachable!();
			};

			let humanized_name = name.to_string_lossy().to_sentence_case();
			if let Err(err) = content_defs.new_item(path, humanized_name, info.def)
				&& !matches!(err, vfs::VfsError::ItemAlreadyExists(_))
			{
				error!("Failed to register asset {}: {err}", info.file.display());
			}
		}
	}
}

async fn load_asset_defs(
	files_to_load: Vec<DirEntry>,
	output: watch::Sender<(usize, Option<PathBuf>)>,
) -> Vec<ContentInfo> {
	let mut asset_defs = Vec::with_capacity(files_to_load.len());

	for (i, file) in files_to_load.iter().enumerate() {
		let data = common::match_else!(fs::read_to_string(file.path()).await; else err => {
			error!(
				err = format!("{err}"),
				"Failed to load asset: {}",
				file.path().display()
			);
			continue;
		});

		let asset_def = common::match_else!(ron::de::from_str::<Arc<dyn ContentDef>>(&data); else err => {
			error!(
				err = format!("{err}"),
				"Failed to deserialize asset: {}",
				file.path().display()
			);
			continue;
		});

		output.send((i + 1, Some(file.path()))).ok();
		asset_defs.push(ContentInfo {
			file: file.path(),
			def: asset_def,
		});
	}

	asset_defs
}

#[derive(Deref, DerefMut)]
struct AssetLoadModal(widgets::MenuModal);

impl Default for AssetLoadModal {
	fn default() -> Self {
		Self(
			widgets::MenuModal::new()
				.closeable(false)
				.with_proportion_to_window(0.3),
		)
	}
}

fn display_load_progress(
	mut contexts: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
	progress: Option<EditorInternalSingle<&ContentLoadProgress>>,
	mut modal: Local<AssetLoadModal>,
) {
	let Some(progress) = progress else {
		modal.open = false;
		return;
	};

	let (total, rx) = (progress.0, &progress.1);
	let (current, path) = &*rx.borrow();

	if *current != 0
		&& let Some(path) = path
	{
		modal.open = true;

		let ctx = contexts.get_mut();
		let id = egui::Id::new("beditor-editor-asset-load-modal");
		modal.show(ctx, id, |ui| {
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
