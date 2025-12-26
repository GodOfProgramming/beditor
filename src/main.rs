use beditor::EditorPlugin;
use bevy::prelude::*;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
	App::new().add_plugins(EditorPlugin::new()).run();
}
