use super::{BundleDnd, SearchableVfs};
use crate::{
	EditorExtension, EditorUi,
	reg::components::{ComponentRegistry, RegisteredComponent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::{marker::PhantomData, num::NonZeroUsize};
use uuid::uuid;
use widgets::Card;

#[derive(Default)]
pub struct ComponentsUiExtension;

impl EditorExtension for ComponentsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ComponentsUi>();
	}
}

#[derive(Component, Reflect)]
pub struct ComponentsUi {
	components_per_row: usize,
}

impl Default for ComponentsUi {
	fn default() -> Self {
		Self {
			components_per_row: 20,
		}
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	component_registry: Res<'w, ComponentRegistry>,

	searchable_vfs: Local<'s, SearchableVfs>,

	_pd: PhantomData<&'s ()>,
}

impl EditorUi for ComponentsUi {
	const NAME: &str = "Components";

	const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

	const UNIQUE: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, true];

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			component_registry,
			mut searchable_vfs,
			_pd: _,
		} = params;
		let vfs = component_registry.vfs();

		searchable_vfs.search_ui(ui, vfs);

		let num_columns = NonZeroUsize::new(self.components_per_row.max(1)).unwrap();

		searchable_vfs.display_ui(ui, vfs, num_columns, |ui, size, basename, id, type_id| {
			if let Some(component) = component_registry.get(type_id) {
				ui_for_item(ui, size, basename, component, id);
			}
		});
	}
}

fn ui_for_item(
	ui: &mut egui::Ui,
	size: egui::Vec2,
	label: &str,
	component: &RegisteredComponent,
	id: egui::Id,
) {
	ui.dnd_drag_source(id, BundleDnd::AddComponent(component.type_id()), |ui| {
		Card::new(size).with_label(label).show(ui, |ui| {
			ui.label(egui_phosphor_icons::icons::PUZZLE_PIECE.regular());
		});
	});
}
