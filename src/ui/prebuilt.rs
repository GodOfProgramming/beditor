use bevy::prelude::*;
use std::any::TypeId;

pub mod assets;
pub mod components;
pub mod debug;
pub mod editor_view;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod menu_bar;
pub mod prefabs;
pub mod resources;

pub enum InspectorDnd {
  AddComponent(TypeId),
  Custom(Box<dyn Fn(&mut World, &[Entity]) + Send + Sync>),
}
