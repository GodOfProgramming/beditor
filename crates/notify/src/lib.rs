use bevy::prelude::*;
use egui_toast::{Toast, ToastKind};
use std::time::Duration;

#[derive(Event)]
pub struct Notification {
	pub(crate) toast: Toast,
	pub(crate) ctx: Option<Box<dyn ToString + Send + Sync>>,
}

impl Notification {
	pub fn new(toast: Toast) -> Self {
		Self { toast, ctx: None }
	}

	pub fn success(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Success, text),
			ctx: None,
		}
	}

	pub fn info(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Info, text),
			ctx: None,
		}
	}

	pub fn warn(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Warning, text),
			ctx: None,
		}
	}

	pub fn error(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Error, text),
			ctx: None,
		}
	}

	pub fn with_context(mut self, ctx: impl ToString + Send + Sync + 'static) -> Self {
		self.ctx = Some(Box::new(ctx));
		self
	}
}

impl Command for Notification {
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}

fn toast(kind: ToastKind, text: impl Into<egui::WidgetText>) -> Toast {
	Toast::new()
		.text(text)
		.kind(kind)
		.options(egui_toast::ToastOptions::default().duration(Duration::from_secs(5)))
}

pub struct NotificationPlugin;

impl Plugin for NotificationPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<Toasts>()
			.add_observer(on_notification)
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

fn on_notification(event: On<Notification>, toasts: Option<ResMut<Toasts>>) {
	if let Some(mut toasts) = toasts {
		toasts.add(event.toast.clone());
	}

	match event.toast.kind {
		egui_toast::ToastKind::Info => {
			if let Some(ctx) = &event.ctx {
				info!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				info!("{}", event.toast.text.text())
			}
		}
		egui_toast::ToastKind::Warning => {
			if let Some(ctx) = &event.ctx {
				warn!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				warn!("{}", event.toast.text.text())
			}
		}
		egui_toast::ToastKind::Error => {
			if let Some(ctx) = &event.ctx {
				warn!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				warn!("{}", event.toast.text.text())
			}
		}
		_ => (),
	}
}
