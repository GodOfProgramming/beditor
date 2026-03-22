use async_std::fs;
use async_walkdir::{DirEntry, Filtering};
use bevy::{
	prelude::*,
	tasks::{
		IoTaskPool, Task,
		futures::check_ready,
		futures_lite::{StreamExt, stream},
	},
	time::common_conditions::on_timer,
};
use macros::EditorAsset;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::PathBuf, time::Duration};
use tokio::sync::watch;

#[derive(Default)]
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, load_all_content).add_systems(
			FixedUpdate,
			(
				file_count_task_poll,
				poll_load_progress.run_if(on_timer(Duration::from_secs(1))),
				poll_asset_load,
			),
		);
	}
}

#[typetag::serde(tag = "ns", content = "data")]
pub trait AssetDef: 'static + Send + Sync + AssetHandlers {}

pub trait AssetHandlers {
	fn on_drag_into_world(&self, world: &mut World);
}

#[derive(Reflect, Serialize, Deserialize, EditorAsset)]
#[ns("editor")]
enum EditorAssetDefs {
	Audio(),
}

impl AssetHandlers for EditorAssetDefs {
	fn on_drag_into_world(&self, world: &mut World) {
		match self {
			EditorAssetDefs::Audio() => todo!(),
		}
	}
}

#[derive(Component)]
struct AssetLoadProgress(
	Task<(
		usize,
		watch::Receiver<(usize, Option<PathBuf>)>,
		Task<Vec<Box<dyn AssetDef>>>,
	)>,
);

#[derive(Component)]
struct AssetProgress(usize, watch::Receiver<(usize, Option<PathBuf>)>);

#[derive(Component)]
struct AssetLoadTask(Task<Vec<Box<dyn AssetDef>>>);

fn load_all_content(mut commands: Commands, assets: Res<AssetServer>) {
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

	commands.spawn(AssetLoadProgress(task));
}

fn file_count_task_poll(
	mut commands: Commands,
	mut q_tasks: Query<(Entity, &mut AssetLoadProgress)>,
) {
	for (entity, mut task) in &mut q_tasks {
		let Some((file_count, progress_output, load_task)) = check_ready(&mut task.0) else {
			continue;
		};

		commands.spawn(AssetProgress(file_count, progress_output));
		commands.spawn(AssetLoadTask(load_task));
		commands.entity(entity).despawn();
	}
}

fn poll_asset_load(mut q_tasks: Query<(Entity, &mut AssetLoadTask)>) {
	for (entity, mut task) in &mut q_tasks {
		let Some(asset_defs) = check_ready(&mut task.0) else {
			continue;
		};
	}
}

async fn load_asset_defs(
	files_to_load: Vec<DirEntry>,
	output: watch::Sender<(usize, Option<PathBuf>)>,
) -> Vec<Box<dyn AssetDef>> {
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

		let asset_def = common::match_else!(ron::de::from_str::<Box<dyn AssetDef>>(&data); else err => {
			error!(
				err = format!("{err}"),
				"Failed to deserialize asset: {}",
				file.path().display()
			);
			continue;
		});

		output.send((i + 1, Some(file.path()))).ok();
		asset_defs.push(asset_def);
	}

	asset_defs
}

fn poll_load_progress() {}
