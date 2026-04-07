mod load;

use crate::private::ui::EditorUiEguiContextPass;
use bevy::{asset::AssetPath, prelude::*};
use derive_new::new;
use macros::EditorAsset;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Default)]
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
	fn build(&self, app: &mut App) {
		let (tx, rx) = watch::channel((0, None));
		app
			.init_resource::<ContentDefs>()
			.init_asset::<ContentDefAsset>()
			.register_asset_loader(load::ContentDefLoader::new(tx))
			.insert_resource(load::ContentDefProgress::new(rx))
			.add_systems(Startup, load::load_content)
			.add_systems(
				FixedUpdate,
				(load::poll_file_count_task, load::poll_content_loading),
			)
			.add_systems(EditorUiEguiContextPass, load::display_load_progress);
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ContentDefs(vfs::Vfs<Handle<ContentDefAsset>>);

#[derive(new, Asset, TypePath, Deref)]
pub struct ContentDefAsset {
	#[deref]
	def: Arc<dyn ContentDef>,
	asset_path: AssetPath<'static>,
}

#[typetag::serde(tag = "ns", content = "data")]
pub trait ContentDef: 'static + Send + Sync + ContentHandlers {}

pub trait ContentHandlers {
	fn insert(&self, entity: Entity, world: &mut World);
}

pub trait ContentUtils {
	fn spawn(&self, world: &mut World) -> Entity;
}

impl<T> ContentUtils for T
where
	Self: ContentHandlers,
{
	fn spawn(&self, world: &mut World) -> Entity {
		let ent = world.spawn_empty().id();
		self.insert(ent, world);
		ent
	}
}

#[derive(Reflect, Serialize, Deserialize)]
pub enum AssetRef {
	Uuid(Uuid),
	File(PathBuf),
	AssetPath(AssetPath<'static>),
}

impl AssetRef {
	pub fn get_handle<A: Asset>(&self, world: &mut World) -> Handle<A> {
		match self {
			Self::Uuid(uuid) => Handle::from(*uuid),
			Self::File(path) => world.load_asset(AssetPath::from_path(path)),
			Self::AssetPath(asset_path) => world.load_asset(asset_path),
		}
	}
}

#[derive(Reflect, Serialize, Deserialize, EditorAsset)]
#[ns("editor")]
pub enum EditorAssetDefs {
	Audio { source: PathBuf },
}

impl ContentHandlers for EditorAssetDefs {
	fn insert(&self, entity: Entity, world: &mut World) {
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
