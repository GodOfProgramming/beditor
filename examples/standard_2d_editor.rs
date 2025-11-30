use beditor::prelude::*;
use bevy::{color::palettes::css::PURPLE, prelude::*};

fn main() {
  App::new()
    .add_plugins(EditorPlugin::new().register_game_camera::<GameCamera>())
    .add_systems(Startup, startup)
    .run();
}

#[derive(Component, Reflect, Identifiable)]
#[id("9d8c1906-62f7-4aab-ab26-cb7f7e8828c6")]
struct GameCamera;

fn startup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ColorMaterial>>,
) {
  commands.spawn((Name::new("Game Camera"), GameCamera, Camera2d));

  commands.spawn((
    Name::new("Purple Square Sprite"),
    Sprite {
      color: Color::from(PURPLE),
      ..default()
    },
    Transform::default()
      .with_translation(Vec3::new(-64.0, 0.0, 0.0))
      .with_scale(Vec3::new(32.0, 32.0, 1.0)),
  ));

  commands.spawn((
    Name::new("Purple Square"),
    Mesh2d(meshes.add(Rectangle::default())),
    MeshMaterial2d(materials.add(Color::from(PURPLE))),
    Transform::default()
      .with_translation(Vec3::new(64.0, 0.0, 0.0))
      .with_scale(Vec3::new(32.0, 32.0, 1.0)),
  ));
}
