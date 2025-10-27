use bevy::{
  asset::{AssetLoader, LoadContext, LoadedFolder, io::Reader},
  ecs::system::{SystemParam, SystemState},
  platform::collections::HashMap,
  prelude::*,
  reflect::GetTypeRegistration,
};
use serde::Deserialize;
use std::marker::PhantomData;

use crate::util;

pub struct PrefabPlugin<T> {
  _pd: PhantomData<T>,
}

impl<T> Default for PrefabPlugin<T> {
  fn default() -> Self {
    Self { _pd: default() }
  }
}

impl<T> Plugin for PrefabPlugin<T>
where
  T: Prefab,
{
  fn build(&self, app: &mut App) {
    app
      .init_asset::<T::Descriptor>()
      .add_event::<PrefabLoadedEvent<T>>()
      .register_asset_loader(PrefabLoader::<T>::default())
      // on startup create a prefab loader
      .add_systems(Startup, Self::on_start)
      // then read all events that come in for the loaded prefab
      .add_systems(Update, (Self::on_load, Self::on_prefab_loaded));
  }
}

impl<T> PrefabPlugin<T>
where
  T: Prefab,
{
  fn on_start(assets: ResMut<AssetServer>, mut commands: Commands) {
    let handle = assets.load_folder(T::DIR);
    commands.insert_resource(PrefabFolder::<T>::new(handle));
    info!("Started folder load for {}", util::short_name_of::<T>());
  }

  fn on_load(
    mut event_reader: EventReader<AssetEvent<LoadedFolder>>,
    folder: Res<PrefabFolder<T>>,
    loaded_folders: Res<Assets<LoadedFolder>>,
    mut event_writer: EventWriter<PrefabLoadedEvent<T>>,
  ) {
    for event in event_reader.read() {
      info!("Loaded folder for {}", util::short_name_of::<T>());
      if event.is_loaded_with_dependencies(folder.handle()) {
        let folders = loaded_folders.get(folder.handle()).unwrap();
        for handle in folders.handles.iter() {
          let id = handle.id().typed_unchecked::<T::Descriptor>();
          event_writer.write(PrefabLoadedEvent::<T>::new(id));
        }
      }
    }
  }

  fn on_prefab_loaded(
    mut event_reader: EventReader<PrefabLoadedEvent<T>>,
    descriptors: Res<Assets<T::Descriptor>>,
    mut prefabs: ResMut<Prefabs>,
    assets: Res<AssetServer>,
  ) {
    for event in event_reader.read() {
      info!(
        "Received prefab load event for {}",
        util::short_name_of::<T>()
      );

      let Some(desc) = descriptors.get(event.id) else {
        warn!("asset id did not resolve to a descriptor asset");
        return;
      };

      let prefab = T::transform(desc, &assets);
      prefabs.register(prefab);
    }
  }
}

type RegistrationFn = dyn Fn(&mut World) -> Box<SpawnFn> + Send + Sync;

#[derive(Resource, Default)]
pub struct PrefabRegistrar {
  registrations: HashMap<String, Box<RegistrationFn>>,
}

impl PrefabRegistrar {
  pub fn register<T>(&mut self)
  where
    T: StaticPrefab,
  {
    self.register_internal(util::short_name_of::<T>(), |world| {
      let mut state = SystemState::<T::Params<'_, '_>>::new(world);

      // the below is what's stored in Prefabs
      move |world| {
        let entity_id = world.spawn_empty().id();
        let params = state.get_mut(world);
        let bundle = T::spawn(entity_id, params);
        world.entity_mut(entity_id).insert(bundle);
      }
    });
  }

  /// Calls R which produces a closure S that is later invoked to return the spawn function
  fn register_internal<R, S>(&mut self, name: impl Into<String>, f: R)
  where
    S: FnMut(&mut World) + Send + Sync + 'static,
    R: Fn(&mut World) -> S + Send + Sync + 'static,
  {
    self
      .registrations
      .insert(name.into(), Box::new(move |world| Box::new((f)(world))));
  }
}

type SpawnFn = dyn FnMut(&mut World) + Send + Sync;
type PrefabSpawnMap = HashMap<String, Box<SpawnFn>>;

#[derive(Resource, Deref, DerefMut)]
pub struct Prefabs(PrefabSpawnMap);

impl Prefabs {
  pub fn new(world: &mut World, registrar: PrefabRegistrar) -> Self {
    let prefabs = registrar
      .registrations
      .into_iter()
      .map(|(k, v)| (k, (v)(world)))
      .collect();

    Self(prefabs)
  }

  fn register<T>(&mut self, prefab: T)
  where
    T: Prefab,
  {
    self.insert(
      prefab.name().to_string(),
      Box::new(move |world| {
        world.spawn(prefab.clone());
      }),
    );
  }

  pub fn spawn(&mut self, id: impl AsRef<str>, world: &mut World) {
    if let Some(spawn_fn) = self.get_mut(id.as_ref()) {
      (spawn_fn)(world);
    }
  }
}

pub trait StaticPrefab: GetTypeRegistration + Sized {
  type Params<'w, 's>: for<'world, 'system> SystemParam<
    Item<'world, 'system> = Self::Params<'world, 'system>,
  >;

  fn spawn(id: Entity, params: Self::Params<'_, '_>) -> impl Bundle;
}

pub trait Prefab: GetTypeRegistration + Bundle + Clone {
  const DIR: &str;
  const EXTENSIONS: &[&str];

  type Descriptor: Asset + for<'a> Deserialize<'a>;

  fn name(&self) -> &str {
    util::short_name_of::<Self>()
  }

  fn transform(desc: &Self::Descriptor, assets: &AssetServer) -> Self;
}

pub struct PrefabLoader<T>
where
  T: Prefab,
{
  _phantom_data: PhantomData<T>,
}

impl<T> Default for PrefabLoader<T>
where
  T: Prefab,
{
  fn default() -> Self {
    Self {
      _phantom_data: default(),
    }
  }
}

impl<T> AssetLoader for PrefabLoader<T>
where
  T: Prefab,
{
  type Asset = <T as Prefab>::Descriptor;

  type Settings = ();

  type Error = std::io::Error;

  async fn load(
    &self,
    reader: &mut dyn Reader,
    _settings: &Self::Settings,
    _load_context: &mut LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let ron: <T as Prefab>::Descriptor = ron::de::from_bytes(&bytes).unwrap();
    Ok(ron)
  }

  fn extensions(&self) -> &[&str] {
    T::EXTENSIONS
  }
}

#[derive(Resource)]
pub struct PrefabFolder<T>
where
  T: Prefab,
{
  handle: Handle<LoadedFolder>,
  _phantom_data: PhantomData<T>,
}

impl<T> PrefabFolder<T>
where
  T: Prefab,
{
  pub fn new(handle: Handle<LoadedFolder>) -> Self {
    Self {
      handle,
      _phantom_data: default(),
    }
  }

  pub fn handle(&self) -> &Handle<LoadedFolder> {
    &self.handle
  }
}

#[derive(Event)]

pub struct PrefabLoadedEvent<T>
where
  T: Prefab,
{
  pub id: AssetId<T::Descriptor>,
}

impl<T> PrefabLoadedEvent<T>
where
  T: Prefab,
{
  pub fn new(id: AssetId<T::Descriptor>) -> Self {
    Self { id }
  }
}
