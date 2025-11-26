use beditor::{assets::StaticPrefab, prelude::*, uuid};
use bevy::{ecs::system::SystemParam, prelude::*};

fn main() {
  let mut editor = Editor::default();

  editor
    .register_game_camera::<GameCamera>()
    .register_static_prefab::<Cube>()
    .add_systems(Startup, startup);

  editor.run();
}

#[derive(Component, Reflect, Default, Identifiable)]
#[id("d0d75fdd-e1b9-4eac-86fc-6eaab8865bad")]
struct GameCamera;

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  // circular base
  commands.spawn((
    Name::new("Base"),
    Mesh3d(meshes.add(Circle::new(4.0))),
    MeshMaterial3d(materials.add(Color::WHITE)),
    Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
  ));
  // cube
  commands.spawn((
    Name::new("Cube"),
    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
  ));
  // light
  commands.spawn((
    Name::new("Light"),
    PointLight {
      shadows_enabled: true,
      ..default()
    },
    Transform::from_xyz(4.0, 8.0, 4.0),
  ));
  // camera
  commands.spawn((
    Name::new("Game Camera"),
    GameCamera,
    Camera3d::default(),
    Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
  ));
}

#[derive(Reflect)]
struct Cube;

struct Spiral {
  theta: f32,
  r: f32,
  h: f32,
}

impl Default for Spiral {
  fn default() -> Self {
    Self {
      theta: 0.0,
      r: 2.0,
      h: 0.0,
    }
  }
}

#[derive(SystemParam)]
struct CubeParams<'w, 's> {
  meshes: ResMut<'w, Assets<Mesh>>,
  materials: ResMut<'w, Assets<StandardMaterial>>,
  spiral: Local<'s, Spiral>,
}

impl StaticPrefab for Cube {
  type Params<'w, 's> = CubeParams<'w, 's>;

  fn spawn(_id: Entity, mut params: Self::Params<'_, '_>) -> impl Bundle {
    let offset = Vec2::new(
      params.spiral.r * params.spiral.theta.cos(),
      params.spiral.r * params.spiral.theta.sin(),
    );

    params.spiral.theta += 30.0f32.to_radians();
    params.spiral.h += 0.5;

    (
      Mesh3d(params.meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
      MeshMaterial3d(params.materials.add(Color::srgb_u8(124, 144, 255))),
      Transform::from_xyz(offset.x, params.spiral.h, offset.y),
    )
  }
}
