use bevy::prelude::*;
use egui_toast::{Toast, ToastKind};
use std::fmt::Debug;

pub struct NotificationPlugin;

impl Plugin for NotificationPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<Toasts>()
			.add_observer(Notification::handle)
			.add_systems(bevy_egui::EguiPrimaryContextPass, show_toasts);
	}
}

#[derive(Resource, Deref, DerefMut)]
struct Toasts(egui_toast::Toasts);

impl Default for Toasts {
	fn default() -> Self {
		Self(
			egui_toast::Toasts::new()
				.anchor(egui::Align2::CENTER_BOTTOM, (0.0, -10.0))
				.direction(egui::Direction::BottomUp),
		)
	}
}

#[derive(Event)]
pub struct Notification {
	toast: Toast,
	ctx: Option<Box<dyn Debug + Send + Sync>>,
}

impl Notification {
	pub fn new(toast: Toast) -> Self {
		Self { toast, ctx: None }
	}

	pub fn success(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: Toast::new().text(text).kind(ToastKind::Success),
			ctx: None,
		}
	}

	pub fn info(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: Toast::new().text(text).kind(ToastKind::Info),
			ctx: None,
		}
	}

	pub fn warn(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: Toast::new().text(text).kind(ToastKind::Warning),
			ctx: None,
		}
	}

	pub fn error(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: Toast::new().text(text).kind(ToastKind::Error),
			ctx: None,
		}
	}

	pub fn with_context(mut self, ctx: impl Debug + Send + Sync + 'static) -> Self {
		self.ctx = Some(Box::new(ctx));
		self
	}

	fn handle(event: On<Self>, mut toasts: ResMut<Toasts>) {
		toasts.add(event.toast.clone());

		match event.toast.kind {
			egui_toast::ToastKind::Info => {
				if let Some(ctx) = &event.ctx {
					info!(ctx = format!("{:#?}", ctx), "{}", event.toast.text.text())
				} else {
					info!("{}", event.toast.text.text())
				}
			}
			egui_toast::ToastKind::Warning => {
				if let Some(ctx) = &event.ctx {
					warn!(ctx = format!("{:#?}", ctx), "{}", event.toast.text.text())
				} else {
					warn!("{}", event.toast.text.text())
				}
			}
			egui_toast::ToastKind::Error => {
				if let Some(ctx) = &event.ctx {
					warn!(ctx = format!("{:#?}", ctx), "{}", event.toast.text.text())
				} else {
					warn!("{}", event.toast.text.text())
				}
			}
			_ => (),
		}
	}
}

fn show_toasts(
	mut contexts: bevy_egui::EguiContexts,
	mut toasts: ResMut<Toasts>,
	mut logged_egui_failure: Local<bool>,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		if *logged_egui_failure {
			return;
		}

		*logged_egui_failure = true;

		warn!("Failed to get egui context");
		return;
	};

	toasts.show(ctx);
}
