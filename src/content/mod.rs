use std::time::Duration;

use bevy::{
	asset::{AssetLoader, LoadedFolder, ReflectAsset},
	prelude::*,
	time::common_conditions::on_timer,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct MetaAssetPlugin;

impl Plugin for MetaAssetPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<MetaAssetHandles>()
			.init_asset::<MetaAsset>()
			.init_asset_loader::<MetaAssetLoader>()
			.add_observer(on_process_meta_handle)
			.add_systems(Startup, startup)
			.add_systems(
				First,
				monitor_folder_loads
					.run_if(on_timer(Duration::from_secs(1)))
					.run_if(resource_exists::<MetaAssetFolderHandle>),
			);
	}
}

#[derive(Resource, Deref)]
struct MetaAssetFolderHandle(Handle<LoadedFolder>);

#[derive(Resource, Default, Deref, DerefMut)]
struct MetaAssetHandles(Vec<Handle<MetaAsset>>);

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
	let handle = asset_server.load_folder("meta");
	commands.insert_resource(MetaAssetFolderHandle(handle));
}

fn monitor_folder_loads(
	mut commands: Commands,
	mut messages: MessageReader<AssetEvent<LoadedFolder>>,
	folders: ResMut<Assets<LoadedFolder>>,
	meta_folder_handle: Res<MetaAssetFolderHandle>,
) {
	let Some(id) = messages.read().find_map(|msg| {
		if let AssetEvent::LoadedWithDependencies { id } = msg
			&& meta_folder_handle.id() == *id
		{
			Some(id)
		} else {
			None
		}
	}) else {
		return;
	};

	let Some(folder) = folders.get(*id) else {
		return;
	};

	commands.remove_resource::<MetaAssetFolderHandle>();

	for handle in folder.handles.iter().cloned() {
		let Ok(meta_handle) = handle.try_typed::<MetaAsset>() else {
			continue;
		};
		commands.trigger(ProcessMetaHandle(meta_handle));
	}
}

#[derive(Event, Deref)]
struct ProcessMetaHandle(Handle<MetaAsset>);

fn on_process_meta_handle(
	event: On<ProcessMetaHandle>,
	mut assets: ResMut<Assets<MetaAsset>>,
	mut handles: ResMut<MetaAssetHandles>,
) {
	let Some(meta_asset) = assets.get(event.id()) else {
		return;
	};

	if let Some(uuid) = meta_asset.uuid {
		let meta_asset = meta_asset.clone();
		assets.remove(event.id());
		assets.insert(uuid, meta_asset).ok();
	} else {
		handles.push(Handle::clone(&event));
	}
}

#[derive(Asset, Reflect, Clone, Serialize, Deserialize)]
#[reflect(Asset, Serialize, Deserialize)]
pub struct MetaAsset {
	format: PayloadFormat,
	uuid: Option<Uuid>,
	payload: Vec<u8>,
}

#[derive(Reflect, Clone, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum PayloadFormat {
	Ron,
}

#[derive(Default)]
struct MetaAssetLoader;

impl AssetLoader for MetaAssetLoader {
	type Asset = MetaAsset;

	type Settings = ();

	type Error = BevyError;

	async fn load(
		&self,
		reader: &mut dyn bevy::asset::io::Reader,
		_settings: &Self::Settings,
		_load_context: &mut bevy::asset::LoadContext<'_>,
	) -> Result<Self::Asset, Self::Error> {
		let mut buf = Vec::new();
		reader.read_to_end(&mut buf).await?;
		let meta_asset = ron::de::from_bytes(&buf)?;
		Ok(meta_asset)
	}

	fn extensions(&self) -> &[&str] {
		&["meta.ron"]
	}
}

#[reflect_trait]
pub trait MetaAssetPayload {}
