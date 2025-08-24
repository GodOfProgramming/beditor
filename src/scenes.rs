use bevy::{asset::ReflectHandle, prelude::*, reflect::TypeRegistryArc, tasks::IoTaskPool};
use std::path::PathBuf;

#[derive(Event)]
pub struct SaveEvent(PathBuf);

impl SaveEvent {
  pub fn _new(path: PathBuf) -> Self {
    Self(path)
  }

  pub fn file(&self) -> &PathBuf {
    &self.0
  }

  pub fn handler(&self, world: &mut World) {
    let world_type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut scene_world = World::new();
    scene_world.insert_resource(world_type_registry.clone());

    let world_type_registry = world_type_registry.read();

    let scene_type_registry = world.resource::<SceneTypeRegistry>().clone();
    let scene_type_registry = scene_type_registry.read();

    let scene_marker_id = world.component_id::<SceneMarker>().unwrap();
    let components = world.components();

    for archetype in world
      .archetypes()
      .iter()
      .filter(|a| a.components().any(|c| c == scene_marker_id))
    {
      for entity in archetype.entities() {
        let new_entity_id = scene_world.spawn_empty().id();

        for comp_id in archetype.components() {
          let Some(comp_info) = components.get_info(comp_id) else {
            error!("failed to get component info for {}", comp_id.index());
            return;
          };

          let Some(comp_type_id) = comp_info.type_id() else {
            error!("failed to get comp type id of {}", comp_info.name());
            return;
          };

          if !scene_type_registry.contains(comp_type_id) {
            // assume if the type is not present in the type registry it is not meant to be saved
            continue;
          }

          let comp_type_reg = world_type_registry.get(comp_type_id).unwrap();

          info!("serializing {}", comp_type_reg.type_info().type_path());

          let Some(ref_comp) = comp_type_reg.data::<ReflectComponent>() else {
            error!("failed to get reflect component of {}", comp_info.name());
            return;
          };

          if let Some(ref_handle) = comp_type_reg.data::<ReflectHandle>() {
            let entity_ref = world.get_entity(entity.id()).unwrap();
            let dyn_ref = ref_comp.reflect(entity_ref).unwrap();
            let asset_handle = ref_handle
              .downcast_handle_untyped(dyn_ref.as_any())
              .unwrap();
            match asset_handle.path() {
              Some(path) => {
                info!("asset path => {:?}", path);
              }
              _ => {
                continue;
              }
            }
          } else {
            ref_comp.copy(
              world,
              &mut scene_world,
              entity.id(),
              new_entity_id,
              &world_type_registry,
            );
          }
        }
      }
    }

    let scene = DynamicScene::from_world(&scene_world);

    let serialization = scene.serialize(&scene_type_registry).unwrap();
    let filename = self.file().clone();
    IoTaskPool::get()
      .spawn(async move {
        let printable_filename = filename.display().to_string();

        info!("saving scene to {}...", printable_filename);
        if let Some(parent) = filename.parent()
          && let Err(err) = async_std::fs::create_dir_all(parent).await
        {
          error!("failed to create directory '{}': {err}", parent.display());
        }

        if let Err(err) = async_std::fs::write(filename, serialization).await {
          error!("failed to save scene to '{}': {err}", printable_filename);
          return;
        }

        info!("finished saving");
      })
      .detach();
  }
}

#[derive(Event)]
pub struct LoadEvent(PathBuf);

impl LoadEvent {
  pub fn _new(path: PathBuf) -> Self {
    Self(path)
  }

  pub fn file(&self) -> &PathBuf {
    &self.0
  }
}

#[derive(Component)]
pub struct SceneMarker;

#[derive(Default, Deref, DerefMut, Clone, Resource)]
pub struct SceneTypeRegistry(TypeRegistryArc);

pub fn check_for_saves(world: &mut World) {
  world.resource_scope(|world, save_events: Mut<Events<SaveEvent>>| {
    save_events.get_cursor().read(&save_events).for_each(|e| {
      e.handler(world);
    });
  });
}

pub fn check_for_loads(
  mut commands: Commands,
  mut load_events: EventReader<LoadEvent>,
  asset_server: Res<AssetServer>,
) {
  load_events.read().for_each(|e| {
    commands.spawn(DynamicSceneRoot(asset_server.load(e.file().clone())));
  });
}
