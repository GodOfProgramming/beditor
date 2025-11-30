use beditor::EditorPlugin;
use bevy::app::App;

fn main() {
  App::new().add_plugins(EditorPlugin::new()).run();
}
