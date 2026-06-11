use std::{
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

use crossterm::event::{Event, KeyCode, KeyEvent};
use fujicore::{features::render::RenderDescriptors, generated::renders::RenderBase};
use log::error;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect, Size},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, FrameExt, Paragraph},
};
use ratatui_explorer::{FileExplorer, Theme};
use ratatui_image::{
    Resize, StatefulImage,
    thread::{ResizeResponse, ThreadProtocol},
};

use crate::{
    border_title,
    ui::{
        Keybind, accent, border_style, muted,
        tabs::{AppCtx, Buffer, Shadowed, TabBehavior},
        widgets::EditorState,
    },
    workers::{ReqId, device::DeviceCommand, fs::FsCommand},
};

const DRAFT: bool = true;

const KEYBINDS: &[Keybind] = &[
    Keybind {
        keys: "i",
        action: "Load image",
    },
    Keybind {
        keys: "r",
        action: "Render",
    },
    Keybind {
        keys: "↑ ↓ / j k",
        action: "Move field",
    },
    Keybind {
        keys: "(⇧)← →",
        action: "Adjust / jump value",
    },
    Keybind {
        keys: "Home / End",
        action: "Min / max",
    },
    Keybind {
        keys: "Enter",
        action: "Edit value",
    },
    Keybind {
        keys: "u",
        action: "Revert changes",
    },
];

struct WorkingPane {
    buffer: Buffer<Shadowed<RenderBase>>,
    editor: EditorState<RenderBase>,
    rendered: Option<ThreadProtocol>,
    in_flight: Option<ReqId>,
}

#[derive(Default)]
pub struct RenderTabState {
    descriptors: Option<&'static RenderDescriptors>,
    image_path: Option<PathBuf>,
    working: Option<WorkingPane>,
    explorer: Option<FileExplorer>,
    image_req: Option<ReqId>,
}

#[allow(clippy::missing_fields_in_debug)]
impl fmt::Debug for RenderTabState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderTabState")
            .field("has_descriptors", &self.descriptors.is_some())
            .field("image_path", &self.image_path)
            .field("has_working", &self.working.is_some())
            .field("explorer_open", &self.explorer.is_some())
            .field("image_req", &self.image_req)
            .finish()
    }
}

impl RenderTabState {
    fn seed_working(&mut self, ctx: &AppCtx, profile: &RenderBase) {
        let Some(descriptors) = ctx
            .device_snapshot
            .as_ref()
            .and_then(|snap| snap.usb_id.supported_camera())
            .and_then(|camera| camera.render)
        else {
            return;
        };
        let canonical = profile.clone();
        let shadow = descriptors.new_shadow_from(&canonical);
        self.descriptors = Some(descriptors);
        self.working = Some(WorkingPane {
            buffer: Buffer::from(Shadowed { canonical, shadow }),
            editor: EditorState::default(),
            rendered: None,
            in_flight: None,
        });
    }

    fn is_editing(&self) -> bool {
        self.working.as_ref().is_some_and(|w| w.editor.is_editing())
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let Some(descriptors) = self.descriptors else {
            return;
        };
        let Some(working) = self.working.as_mut() else {
            return;
        };
        let _ = working
            .editor
            .handle_key(key, &mut working.buffer, descriptors);
    }

    fn request_render(&mut self, ctx: &AppCtx) {
        let Some(device) = ctx.device.as_ref() else {
            return;
        };
        let Some(working) = self.working.as_mut() else {
            return;
        };
        if working.in_flight.is_some() {
            return;
        }
        let req = ctx.req.next();
        working.in_flight = Some(req);
        device.send(DeviceCommand::Render {
            req,
            base: working.buffer.working.canonical.clone(),
            draft: DRAFT,
        });
    }

    fn revert(&mut self) {
        if let Some(working) = self.working.as_mut() {
            working.buffer.working = working.buffer.fetched.clone();
        }
    }

    fn open_explorer(&mut self) {
        if self.explorer.is_some() {
            return;
        }
        match FileExplorer::new() {
            Ok(mut explorer) => {
                explorer.set_theme(Self::explorer_theme());
                self.explorer = Some(explorer);
            }
            Err(e) => error!("failed to open file picker: {e}"),
        }
    }

    fn handle_explorer_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        let Some(explorer) = self.explorer.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.explorer = None;
                return;
            }
            KeyCode::Enter if !explorer.current().is_dir => {
                let path = explorer.current().path.clone();
                self.explorer = None;
                self.request_image_load(ctx, path);
                return;
            }
            _ => {}
        }
        let _ = explorer.handle(&Event::Key(key));
    }

    fn request_image_load(&mut self, ctx: &AppCtx, path: PathBuf) {
        let req = ctx.req.next();
        self.image_req = Some(req);
        ctx.fs.send(FsCommand::ReadImage { req, path });
    }

    fn draw_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let descriptors = self.descriptors;
        match (descriptors, self.working.as_mut()) {
            (Some(descriptors), Some(working)) => {
                let dirty = working.buffer.dirty();
                working.editor.draw(
                    frame,
                    area,
                    true,
                    "Settings",
                    &working.buffer.working,
                    &working.buffer.fetched.canonical,
                    descriptors,
                    dirty,
                );
            }
            _ => EditorState::<RenderBase>::draw_message(
                frame,
                area,
                false,
                Some("Settings"),
                "Load a reference image to begin.",
                muted(),
            ),
        }
    }

    fn draw_image(&mut self, frame: &mut Frame, area: Rect, suppress: bool) {
        let block = Block::bordered()
            .border_style(border_style(true))
            .title(border_title!(1, "Render"));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if suppress {
            return;
        }

        let needs_image = self.image_path.is_none();
        if let Some(protocol) = self.working.as_mut().and_then(|w| w.rendered.as_mut()) {
            let resize = Resize::Fit(None);
            let target = protocol
                .size_for(resize.clone(), Size::new(inner.width, inner.height))
                .map_or(inner, |fitted| Self::center_rect(inner, fitted));
            frame.render_stateful_widget(
                StatefulImage::<ThreadProtocol>::default().resize(resize),
                target,
                protocol,
            );
        } else {
            let message = if needs_image {
                "Press i to load a reference image."
            } else {
                "Press r to render."
            };
            Self::draw_centered(frame, inner, message);
        }
    }

    pub fn apply_resized(&mut self, response: ResizeResponse) {
        if let Some(protocol) = self.working.as_mut().and_then(|w| w.rendered.as_mut()) {
            protocol.update_resized_protocol(response);
        }
    }

    pub fn on_render_done(&mut self, ctx: &AppCtx, req: ReqId, decoded: image::DynamicImage) {
        let Some(working) = self.working.as_mut() else {
            return;
        };
        if working.in_flight != Some(req) {
            return;
        }
        working.in_flight = None;
        let protocol = ctx.image_picker.new_resize_protocol(decoded);
        working.rendered = Some(ThreadProtocol::new(ctx.resize_tx.clone(), Some(protocol)));
    }

    pub fn on_render_failed(&mut self, req: ReqId) {
        if let Some(working) = self.working.as_mut()
            && working.in_flight == Some(req)
        {
            working.in_flight = None;
        }
    }

    fn explorer_theme() -> Theme {
        Theme::default()
            .with_block(Block::bordered().border_style(border_style(true)))
            .add_default_title()
            .with_dir_style(Style::default().fg(accent()))
            .with_highlight_item_style(Style::default().add_modifier(Modifier::REVERSED))
            .with_highlight_dir_style(
                Style::default()
                    .fg(accent())
                    .add_modifier(Modifier::REVERSED),
            )
    }

    fn center_rect(outer: Rect, size: Size) -> Rect {
        let width = size.width.min(outer.width);
        let height = size.height.min(outer.height);
        Rect {
            x: outer.x + (outer.width - width) / 2,
            y: outer.y + (outer.height - height) / 2,
            width,
            height,
        }
    }

    fn draw_centered(frame: &mut Frame, area: Rect, text: &str) {
        let [line] = Layout::vertical([Constraint::Length(1)])
            .flex(Flex::Center)
            .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text.to_owned(),
                Style::default().fg(muted()),
            )))
            .alignment(Alignment::Center),
            line,
        );
    }

    fn modal_area(area: Rect) -> Rect {
        let [vertical] = Layout::vertical([Constraint::Percentage(70)])
            .flex(Flex::Center)
            .areas(area);
        let [modal] = Layout::horizontal([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .areas(vertical);
        modal
    }
}

impl TabBehavior for RenderTabState {
    fn render(&mut self, ctx: &AppCtx, frame: &mut Frame, area: Rect) {
        let [sidebar, image] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);
        self.draw_sidebar(frame, sidebar);
        self.draw_image(frame, image, ctx.overlay || self.explorer.is_some());

        if let Some(explorer) = self.explorer.as_ref() {
            let area = Self::modal_area(area);
            frame.render_widget(Clear, area);
            frame.render_widget_ref(explorer.widget(), area);
        }
    }

    fn is_capturing_input(&self) -> bool {
        self.explorer.is_some() || self.is_editing()
    }

    fn keybinds(&self) -> &'static [Keybind] {
        KEYBINDS
    }

    fn on_device_disconnected(&mut self, _ctx: &AppCtx) {
        self.descriptors = None;
        self.working = None;
        self.image_path = None;
        self.image_req = None;
        self.explorer = None;
    }

    fn on_image_read(&mut self, ctx: &AppCtx, req: ReqId, path: &Path, image: &Arc<[u8]>) {
        if self.image_req != Some(req) {
            return;
        }
        let Some(device) = ctx.device.as_ref() else {
            self.image_req = None;
            return;
        };
        let load_req = ctx.req.next();
        self.image_req = Some(load_req);
        self.image_path = Some(path.to_path_buf());
        device.send(DeviceCommand::LoadImage {
            req: load_req,
            image: Arc::clone(image),
        });
    }

    fn on_image_read_failed(&mut self, _ctx: &AppCtx, req: ReqId) {
        if self.image_req == Some(req) {
            self.image_req = None;
        }
    }

    fn on_image_loaded(&mut self, ctx: &AppCtx, req: ReqId, profile: &RenderBase) {
        if self.image_req != Some(req) {
            return;
        }
        self.image_req = None;
        self.seed_working(ctx, profile);
    }

    fn on_image_load_failed(&mut self, _ctx: &AppCtx, req: ReqId) {
        if self.image_req == Some(req) {
            self.image_req = None;
            self.image_path = None;
        }
    }

    fn on_key(&mut self, ctx: &AppCtx, key: KeyEvent) {
        if self.explorer.is_some() {
            self.handle_explorer_key(ctx, key);
            return;
        }
        if self.is_editing() {
            self.handle_editor_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('i') => self.open_explorer(),
            KeyCode::Char('r') => self.request_render(ctx),
            KeyCode::Char('u') => self.revert(),
            _ => self.handle_editor_key(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;
    use fujicore::generated::cameras::SUPPORTED;
    use ratatui_image::picker::Picker;

    use super::*;
    use crate::workers::{
        ReqIdGen,
        device::DeviceSnapshot,
        fs::{FsHandle, backup::BackupLibrarySnapshot, simulation::SimulationLibrarySnapshot},
    };

    fn render_ctx() -> AppCtx {
        let camera = SUPPORTED
            .iter()
            .find(|c| c.render.is_some())
            .expect("a render-capable camera exists");
        let (tx, _rx) = unbounded();
        let dir = tempfile::tempdir().unwrap().keep();
        let fs = FsHandle::spawn(dir.join("simulations"), dir.join("backups"), tx);
        AppCtx {
            device: None,
            fs,
            req: ReqIdGen::new(),
            device_snapshot: Some(DeviceSnapshot {
                name: camera.name,
                usb_id: camera.usb_id,
                bus_address: "0:0".to_owned(),
                battery: 100,
                capabilities: &[],
            }),
            simulation_library_snapshot: SimulationLibrarySnapshot::empty(),
            backup_library_snapshot: BackupLibrarySnapshot::empty(),
            image_picker: Picker::halfblocks(),
            resize_tx: std::sync::mpsc::channel().0,
            overlay: false,
        }
    }

    fn sample_profile() -> RenderBase {
        let camera = SUPPORTED
            .iter()
            .find(|c| c.render.is_some())
            .expect("a render-capable camera exists");
        camera.render.unwrap().new_canonical_default()
    }

    fn load_profile(tab: &mut RenderTabState, ctx: &AppCtx) {
        let req = ctx.req.next();
        tab.image_req = Some(req);
        tab.on_image_loaded(ctx, req, &sample_profile());
    }

    #[test]
    fn editor_is_empty_until_a_profile_is_loaded() {
        let ctx = render_ctx();
        let mut tab = RenderTabState::default();
        tab.on_device_disconnected(&ctx);
        assert!(tab.working.is_none());
        load_profile(&mut tab, &ctx);
        assert!(tab.working.is_some());
        assert!(tab.descriptors.is_some());
    }

    #[test]
    fn disconnect_clears_state() {
        let ctx = render_ctx();
        let mut tab = RenderTabState::default();
        load_profile(&mut tab, &ctx);
        tab.on_device_disconnected(&ctx);
        assert!(tab.working.is_none());
        assert!(tab.descriptors.is_none());
        assert!(tab.image_path.is_none());
    }

    #[test]
    fn stale_render_result_is_discarded() {
        let ctx = render_ctx();
        let mut tab = RenderTabState::default();
        load_profile(&mut tab, &ctx);
        let req = ctx.req.next();
        tab.working.as_mut().unwrap().in_flight = Some(req);
        tab.on_render_done(&ctx, ctx.req.next(), image::DynamicImage::new_rgb8(1, 1));
        assert_eq!(tab.working.as_ref().unwrap().in_flight, Some(req));
    }

    #[test]
    fn matching_render_clears_in_flight() {
        let ctx = render_ctx();
        let mut tab = RenderTabState::default();
        load_profile(&mut tab, &ctx);
        let req = ctx.req.next();
        tab.working.as_mut().unwrap().in_flight = Some(req);
        tab.on_render_done(&ctx, req, image::DynamicImage::new_rgb8(1, 1));
        assert_eq!(tab.working.as_ref().unwrap().in_flight, None);
    }

    #[test]
    fn stale_image_loaded_is_ignored() {
        let ctx = render_ctx();
        let mut tab = RenderTabState::default();
        tab.request_image_load(&ctx, PathBuf::from("/pending.RAF"));
        tab.on_image_loaded(&ctx, ctx.req.next(), &sample_profile());
        assert!(tab.working.is_none());
    }
}
