//! Device update preferences and host-owned status. No transport or filesystem work lives here.
use crate::{MessagingUi, design, icons};

/// One release in the update log, already reduced to plain text by the host.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogEntry {
	pub version: String,
	/// `YYYY-MM-DD`, empty when the release has no publish date.
	pub date: String,
	/// Section headings ("Features", "Bug Fixes") with their bullet points.
	pub sections: Vec<(String, Vec<String>)>,
}

pub struct Updates {
	pub auto_update: bool,
	pub nightly: bool,
	pub status: String,
	pub busy: bool,
	pub available: bool,
	pub ready: bool,
	pub supported: bool,
	pub flatpak: bool,
	pub linux_update_cmd: Option<String>,
	pub progress: Option<f32>,
	pub check_requested: bool,
	pub download_requested: bool,
	pub restart_requested: bool,
	pub copied_diagnostics: Option<f64>,
	/// Feedback for the copy button of the diagnostic log's recent lines.
	pub copied_log: Option<f64>,
	pub copied_command: Option<f64>,
	/// Recent releases on the selected channel, newest first.
	pub log: Vec<LogEntry>,
}
impl Default for Updates {
	fn default() -> Self {
		Self {
			auto_update: false,
			nightly: true,
			status: "Updates have not been checked yet.".into(),
			busy: false,
			available: false,
			ready: false,
			supported: false,
			flatpak: false,
			linux_update_cmd: None,
			progress: None,
			check_requested: false,
			download_requested: false,
			restart_requested: false,
			copied_diagnostics: None,
			copied_log: None,
			copied_command: None,
			log: Vec::new(),
		}
	}
}
impl MessagingUi {
	/// Gateway-recovery notice stacked into the account card, styled as a tinted row that
	/// reads as a distinct, tappable banner. The whole row requests an immediate reconnect.
	pub(super) fn reconnecting_banner(&mut self, ui: &mut egui::Ui, first: bool) {
		let colors = design::palette(ui);
		let label = crate::i18n::translate("reconnecting");
		let action = crate::i18n::translate("reconnect-now");
		let (rect, response) =
			ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::click());
		let hovered = response.hovered() || response.has_focus();
		let top = if first { 8 } else { 0 };
		ui.painter().rect_filled(
			rect,
			egui::CornerRadius {
				nw: top,
				ne: top,
				sw: 0,
				se: 0,
			},
			colors
				.warning
				.gamma_multiply(if hovered { 0.18 } else { 0.12 }),
		);
		let mut content = ui.new_child(
			egui::UiBuilder::new()
				.max_rect(rect.shrink2(egui::vec2(10.0, 0.0)))
				.layout(egui::Layout::left_to_right(egui::Align::Center)),
		);
		let ui = &mut content;
		ui.spacing_mut().item_spacing.x = 8.0;
		let (mark, _) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
		icons::paint(ui.painter(), icons::Icon::Reload, mark, colors.warning);
		ui.add(
			egui::Label::new(design::medium(ui, &label, 12.0).color(colors.warning))
				.truncate()
				.selectable(false),
		);
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			let color = if hovered {
				colors.warning
			} else {
				colors.warning.gamma_multiply(0.75)
			};
			ui.add(
				egui::Label::new(design::medium(ui, &action, 11.0).color(color))
					.truncate()
					.selectable(false),
			);
		});
		response.widget_info(|| {
			egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &action)
		});
		if response.clicked() {
			self.reconnect_requested = true;
		}
	}

	/// Formats system and client environment details for GitHub issue reports.
	pub fn diagnostic_info(&self, ctx: &egui::Context) -> String {
		let os = std::env::consts::OS;
		let arch = std::env::consts::ARCH;
		let channel = match self.build.channel {
			design::Channel::Stable => "Stable",
			design::Channel::Nightly => "Nightly",
			design::Channel::Dev => "Dev",
		};
		let theme_mode = match ctx.theme() {
			egui::Theme::Dark => "Dark",
			egui::Theme::Light => "Light",
		};
		let theme_variant = design::variant().label();
		let scale = ctx.pixels_per_point();
		let update_channel = if self.updates.nightly {
			"Nightly"
		} else {
			"Production"
		};

		#[cfg(target_os = "linux")]
		let session_type = std::env::var("XDG_SESSION_TYPE")
			.map(|s| format!(" ({s})"))
			.unwrap_or_default();
		#[cfg(not(target_os = "linux"))]
		let session_type = "";

		#[cfg(target_os = "linux")]
		let package_type = if self.updates.flatpak {
			"\n- **Packaging:** Flatpak".to_owned()
		} else if let Some(cmd) = &self.updates.linux_update_cmd {
			let mgr = if cmd.contains("dnf") {
				"DNF (RPM)"
			} else if cmd.contains("apt") {
				"APT (DEB)"
			} else if cmd.contains("pacman") {
				"Pacman (Arch)"
			} else if cmd.contains("zypper") {
				"Zypper (RPM)"
			} else {
				"Native Package"
			};
			format!("\n- **Packaging:** {mgr}")
		} else {
			"\n- **Packaging:** Native / AppImage".to_owned()
		};
		#[cfg(not(target_os = "linux"))]
		let package_type = "";

		// The adapter and the preference that chose it are what graphics reports hinge on.
		let graphics = if self.gpu_adapter.is_empty() {
			String::new()
		} else {
			format!(
				"\n- **Graphics:** {} ({} preference; requested API: {}{})",
				self.gpu_adapter,
				self.gpu_preference.label(),
				self.startup_graphics_api.label(),
				if self.graphics_api_env_override {
					"; WGPU_BACKEND override"
				} else {
					""
				}
			)
		};

		format!(
			"- **Serein Version:** {} ({channel})\n- **Operating System:** {os} ({arch}){session_type}{package_type}{graphics}\n- **Display Scale:** {scale:.2}\n- **Theme:** {theme_mode} ({theme_variant})\n- **Update Channel:** {update_channel}\n- **Auto Update:** {}",
			self.build.version,
			if self.updates.auto_update {
				"Enabled"
			} else {
				"Disabled"
			}
		)
	}

	/// Copies formatted diagnostics to clipboard and sets a temporary feedback countdown.
	pub fn copy_diagnostic_info(&mut self, ctx: &egui::Context) {
		let info = self.diagnostic_info(ctx);
		ctx.copy_text(info);
		self.updates.copied_diagnostics = Some(ctx.input(|i| i.time) + 2.5);
		ctx.request_repaint_after(std::time::Duration::from_secs(3));
	}

	/// Update controls for the signed-out header, rendered inside its menu popup so the
	/// screen never grows a second, movable window.
	pub fn updates_menu(&mut self, ui: &mut egui::Ui, demo: bool) {
		ui.set_min_width(340.0);
		ui.set_max_width(340.0);
		self.update_settings(ui, demo);
	}

	pub(super) fn update_settings(&mut self, ui: &mut egui::Ui, demo: bool) {
		let colors = design::palette(ui);
		design::card(ui, |ui| {
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 14.0;
				let (badge, _) =
					ui.allocate_exact_size(egui::Vec2::splat(44.0), egui::Sense::hover());
				ui.painter()
					.rect_filled(badge, 12, colors.accent.gamma_multiply(0.16));
				crate::icons::paint(
					ui.painter(),
					crate::icons::Icon::Serein,
					badge.shrink(10.0),
					colors.accent,
				);
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					if self.updates.ready {
						ui.add_enabled_ui(!self.updates.busy, |ui| {
							if design::button(
								ui,
								&crate::i18n::translate(
									"updates-update-settings-restart-to-update",
								),
								design::ButtonKind::Primary,
							)
							.clicked()
							{
								self.updates.restart_requested = true;
							}
						});
					} else if self.updates.available && self.updates.supported {
						ui.add_enabled_ui(!self.updates.busy, |ui| {
							if design::button(
								ui,
								&crate::i18n::translate("updates-update-settings-download-update"),
								design::ButtonKind::Primary,
							)
							.clicked()
							{
								self.updates.download_requested = true;
							}
						});
					} else {
						let allowed = (!cfg!(debug_assertions) || demo) && !self.updates.busy;
						ui.add_enabled_ui(allowed, |ui| {
							if design::button(
								ui,
								&crate::i18n::translate(
									"updates-update-settings-check-for-updates",
								),
								design::ButtonKind::Outline,
							)
							.on_disabled_hover_text(crate::i18n::translate_if_key(
								if cfg!(debug_assertions) && !demo {
									"updates-update-settings-update-checks-are-disabled-in-debug-builds"
								} else {
									"updates-update-settings-finish-the-current-update-before-checking-again"
								},
							))
							.clicked()
							{
								self.updates.check_requested = true;
							}
						});
					}
					if self.updates.busy {
						ui.add(egui::Spinner::new().size(16.0));
					}
					// Reserve the action's width before laying out the status beside it.
					ui.vertical(|ui| {
						ui.spacing_mut().item_spacing.y = 2.0;
						ui.label(
							design::semibold(ui, format!("Serein {}", self.build.version), 17.0)
								.color(colors.text_strong),
						);
						ui.add(
							egui::Label::new(
								egui::RichText::new(&self.updates.status)
									.size(13.0)
									.color(colors.muted),
							)
							.truncate(),
						);
					});
				});
			});
			if let Some(progress) = self.updates.progress {
				ui.add_space(10.0);
				ui.add(
					egui::ProgressBar::new(progress)
						.desired_height(6.0)
						.corner_radius(3)
						.fill(colors.accent),
				);
			}
			if self.updates_save_failed && !demo {
				ui.add_space(10.0);
				design::notice(
					ui,
					design::Level::Warning,
					&crate::i18n::translate(
						"updates-update-settings-could-not-load-or-save-update-preferences-changes-may-not",
					),
				);
			}
		});
		self.update_log(ui);
		design::group(
			ui,
			&crate::i18n::translate("updates-update-settings-preferences"),
			|ui| {
				ui.add_enabled_ui(self.updates.supported || demo, |ui| {
					design::switch(
						ui,
						"updates-update-settings-auto-update",
						Some(
							"updates-update-settings-download-updates-in-the-background-restart-when-you-are-ready",
						),
						&mut self.updates.auto_update,
					);
				});
				design::card_divider(ui);
				design::row(
					ui,
					"updates-update-settings-release-channel",
					Some(if self.updates.nightly {
						"updates-update-settings-early-builds-with-the-newest-changes-nightly-releases-can-be"
					} else {
						"updates-update-settings-published-stable-releases-switching-channels-never-installs-an-older-ver"
					}),
					|ui| {
						crate::design::Select::from_id_salt("update-release-channel")
							.selected_text(crate::i18n::translate_if_key(
								&(if self.updates.nightly {
									crate::i18n::translate("updates-update-settings-nightly")
								} else {
									crate::i18n::translate("updates-update-settings-production")
								}),
							))
							.width(ui.available_width().min(160.0))
							.show_ui(ui, |ui| {
								ui.selectable_value(
									&mut self.updates.nightly,
									false,
									crate::i18n::translate("updates-update-settings-production"),
								);
								ui.selectable_value(
									&mut self.updates.nightly,
									true,
									crate::i18n::translate("updates-update-settings-nightly"),
								);
							});
					},
				);
				if !demo {
					if self.updates.flatpak {
						design::hint(
							ui,
							&crate::i18n::translate(
								"updates-update-settings-flatpak-manages-updates-via-its-repository-run-flatpak-update-or",
							),
						);
					} else if !self.updates.supported {
						if let Some(cmd) = &self.updates.linux_update_cmd {
							design::card_divider(ui);
							let copied_cmd = self
								.updates
								.copied_command
								.is_some_and(|until| ui.input(|i| i.time) < until);
							let cmd = cmd.clone();
							design::row(
								ui,
								"updates-update-settings-package-manager-updates",
								Some(
									"updates-update-settings-serein-was-installed-via-your-distribution-run-this-in-a",
								),
								|ui| {
									if design::button(
										ui,
										&crate::i18n::translate_if_key(if copied_cmd {
											"updates-update-settings-copied"
										} else {
											"updates-update-settings-copy-command"
										}),
										design::ButtonKind::Outline,
									)
									.clicked()
									{
										ui.ctx().copy_text(cmd.clone());
										self.updates.copied_command =
											Some(ui.input(|i| i.time) + 2.5);
										ui.ctx().request_repaint_after(
											std::time::Duration::from_secs(3),
										);
									}
								},
							);
							ui.add_space(6.0);
							egui::Frame::new()
								.fill(colors.base)
								.corner_radius(6)
								.inner_margin(egui::Margin::symmetric(10, 6))
								.show(ui, |ui| {
									ui.set_width(ui.available_width());
									ui.monospace(&cmd);
								});
						} else {
							design::hint(
								ui,
								&crate::i18n::translate(
									"updates-update-settings-in-app-installation-requires-a-macos-or-windows-release-package",
								),
							);
						}
					}
				}
			},
		);
		design::group(
			ui,
			&crate::i18n::translate("updates-update-settings-support-diagnostics"),
			|ui| {
				let copied = self
					.updates
					.copied_diagnostics
					.is_some_and(|until| ui.input(|i| i.time) < until);
				let mut copying = false;
				let mut saving = false;
				design::row(
					ui,
					"updates-update-settings-issue-diagnostics",
					Some(
						"updates-update-settings-copy-system-and-client-environment-details-formatted-for-github-issue",
					),
					|ui| {
						if design::button(
							ui,
							&crate::i18n::translate("updates-update-settings-save-diagnostics"),
							design::ButtonKind::Outline,
						)
						.clicked()
						{
							saving = true;
						}
						if design::button(
							ui,
							&crate::i18n::translate_if_key(if copied {
								"updates-update-settings-copied"
							} else {
								"updates-update-settings-copy"
							}),
							design::ButtonKind::Outline,
						)
						.clicked()
						{
							copying = true;
						}
					},
				);
				if copying {
					self.copy_diagnostic_info(ui.ctx());
				}
				if saving {
					self.diagnostics_save_requested = true;
				}
				if let Some(path) = &self.diagnostics_saved_path {
					// The file the owner should attach; naming it is enough, no new copy needed.
					let colors = design::palette(ui);
					ui.add_space(4.0);
					ui.add(
						egui::Label::new(egui::RichText::new(path).size(12.0).color(colors.muted))
							.wrap(),
					);
				}
				design::card_divider(ui);
				let copied_log = self
					.updates
					.copied_log
					.is_some_and(|until| ui.input(|i| i.time) < until);
				design::row(
					ui,
					"updates-update-settings-diagnostic-log",
					Some(if self.diagnostics_log_failed {
						"updates-update-settings-the-log-folder-could-not-be-written"
					} else {
						"updates-update-settings-timings-stages-and-failures-in-one-local-file-no-account-channel-or"
					}),
					|ui| {
						crate::design::Select::from_id_salt("diagnostic-log-level")
							.selected_text(log_level_label(self.diagnostics_log))
							.width(ui.available_width().min(160.0))
							.show_ui(ui, |ui| {
								for level in diagnostics::Level::ALL {
									ui.selectable_value(
										&mut self.diagnostics_log,
										level,
										log_level_label(level),
									);
								}
							});
					},
				);
				if self.diagnostics_log != diagnostics::Level::Off
					&& let Some(path) = &self.diagnostics_log_path
				{
					// The file is named where the owner enables the log, so a report can be found
					// without guessing which of Serein's folders holds it; a warning colour marks
					// a folder the system refused to open, whose path the host copied instead.
					let colors = design::palette(ui);
					let color = if self.diagnostics_log_open_failed {
						colors.warning
					} else {
						colors.muted
					};
					ui.add_space(4.0);
					ui.add(
						egui::Label::new(egui::RichText::new(path).size(12.0).color(color)).wrap(),
					);
				}
				design::card_divider(ui);
				design::row(
					ui,
					"updates-update-settings-copy-recent-lines",
					None,
					|ui| {
						if design::button(
							ui,
							&crate::i18n::translate_if_key(if copied_log {
								"updates-update-settings-copied"
							} else {
								"updates-update-settings-copy"
							}),
							design::ButtonKind::Outline,
						)
						.clicked()
						{
							ui.ctx().copy_text(diagnostics::recent(200));
							self.updates.copied_log = Some(ui.input(|i| i.time) + 2.5);
							ui.ctx()
								.request_repaint_after(std::time::Duration::from_secs(3));
						}
						if design::button(
							ui,
							&crate::i18n::translate("updates-update-settings-open-folder"),
							design::ButtonKind::Outline,
						)
						.clicked()
						{
							self.open_log_folder_requested = true;
						}
					},
				);
			},
		);
	}
}

/// The interface name of each diagnostic log level.
fn log_level_label(level: diagnostics::Level) -> String {
	crate::i18n::translate(match level {
		diagnostics::Level::Off => "updates-update-settings-log-off",
		diagnostics::Level::Error => "updates-update-settings-log-errors",
		diagnostics::Level::Warn => "updates-update-settings-log-warnings",
		diagnostics::Level::Info => "updates-update-settings-log-events",
		diagnostics::Level::Debug => "updates-update-settings-log-everything",
	})
}

impl MessagingUi {
	/// What changed between the running build and the newest release; hidden when up to date.
	fn update_log(&self, ui: &mut egui::Ui) {
		if self.updates.log.is_empty() {
			return;
		}
		let colors = design::palette(ui);
		design::group(ui, &crate::i18n::translate("updates-update-log"), |ui| {
			for (index, entry) in self.updates.log.iter().enumerate() {
				if index > 0 {
					design::card_divider(ui);
				}
				let title = format!("Serein {}", entry.version);
				egui::CollapsingHeader::new(
					design::semibold(ui, &title, 14.0).color(colors.text_strong),
				)
				.id_salt(("update-log", &entry.version))
				.default_open(index == 0)
				.show(ui, |ui| {
					if !entry.date.is_empty() {
						ui.label(
							egui::RichText::new(&entry.date)
								.size(12.0)
								.color(colors.muted),
						);
					}
					for (heading, items) in &entry.sections {
						if !heading.is_empty() {
							ui.add_space(4.0);
							ui.label(design::semibold(ui, heading, 13.0).color(colors.text_strong));
						}
						for item in items {
							ui.horizontal_top(|ui| {
								ui.label(egui::RichText::new("•").size(13.0).color(colors.muted));
								ui.add(
									egui::Label::new(egui::RichText::new(item).size(13.0)).wrap(),
								);
							});
						}
					}
				});
			}
		});
	}
}
