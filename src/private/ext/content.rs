use std::{num::NonZeroUsize, sync::Arc};

use bevy::{ecs::system::SystemParam, prelude::*};
use widgets::Card;

use crate::{
	AssetDef, EditorExtension, EditorUi,
	content::AssetDefs,
	private::ext::{EntityDnd, SearchableVfs},
};

#[derive(Default)]
pub struct ContentUiExtension;

impl EditorExtension for ContentUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ContentUi>();
	}
}

#[derive(Component)]
pub struct ContentUi;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	asset_defs: Res<'w, AssetDefs>,
	searchable_vfs: Local<'s, SearchableVfs>,
}

impl EditorUi for ContentUi {
	const NAME: &str = "Content";

	const ID: uuid::Uuid = uuid::uuid!("73835581-7c79-494a-a191-f0b4922cdbfc");

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			asset_defs,
			mut searchable_vfs,
		} = params;
		let vfs = &**asset_defs;

		searchable_vfs.search_ui(ui, vfs);

		let num_columns = NonZeroUsize::new(10).unwrap();

		searchable_vfs.display_ui(ui, vfs, num_columns, |ui, size, basename, id, asset_def| {
			ui_for_item(ui, size, basename, Arc::clone(asset_def), id);
		});
	}
}

fn ui_for_item(
	ui: &mut egui::Ui,
	size: egui::Vec2,
	label: &str,
	asset_def: Arc<dyn AssetDef>,
	id: egui::Id,
) {
	ui.dnd_drag_source(id, EntityDnd::AddAsset(asset_def), |ui| {
		Card::new(size).with_label(label).show(ui, |ui| {
			ui.label(egui_phosphor_icons::icons::CUBE.regular());
		});
	});
}
