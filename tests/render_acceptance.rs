//! Requires a real GPU/backend and installed monospace fonts. Run explicitly for visual evidence.
use juna::{gpu::block_on, render::Renderer, vt::TerminalState};

#[test]
#[ignore = "real GPU; status footer capture and terminal-region preservation"]
fn git_status_footer_is_owned_and_does_not_cover_terminal_rows() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut renderer = Renderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        2048,
        (10., 22.),
        8.,
        16.,
    )
    .unwrap();
    renderer.set_top_inset(juna::app::ui::TAB_HEIGHT);
    let theme = juna::theme::builtins()
        .into_iter()
        .find(|t| t.id == "catppuccin-mocha")
        .unwrap();
    renderer.apply_theme(&theme, 4.5).unwrap();
    let mut terminal = TerminalState::new(90, 19);
    terminal.advance(b"Terminal rows stay visible\r\n\x1b[?25l");
    let cells = terminal.snapshot(0).cells;
    let scene = juna::app::ui::status_scene(
        "Git: main · 2 changed · 1 conflicts · ↑1 ↓2 (last fetch)",
        1024.,
        512.,
        1.,
        &theme,
    );
    let baseline = render_scene_at(
        &device,
        &queue,
        &mut renderer,
        &terminal,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        None,
        (1024, 512),
    );
    let pixels = render_scene_at(
        &device,
        &queue,
        &mut renderer,
        &terminal,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some(&scene),
        (1024, 512),
    );
    assert_eq!(&pixels[..486 * 1024 * 4], &baseline[..486 * 1024 * 4]);
    assert_ne!(&pixels[486 * 1024 * 4..], &baseline[486 * 1024 * 4..]);
    assert_eq!(terminal.snapshot(0).cells, cells);
    let mut ppm = b"P6\n1024 512\n255\n".to_vec();
    for pixel in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::create_dir_all("target/acceptance").unwrap();
    std::fs::write("target/acceptance/git-status.ppm", ppm).unwrap();
}

#[test]
#[ignore = "real GPU; captures all bundled fonts and checks clipping/fallback"]
fn bundled_fonts_render_and_resize_without_row_bleed() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    std::fs::create_dir_all("target/acceptance/fonts").unwrap();
    for font in juna::fonts::BUNDLED {
        let mut renderer = Renderer::new_with_family(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            2048,
            (12., 24.),
            8.,
            16.,
            font.name,
        )
        .unwrap();
        let mut term = TerminalState::new(40, 8);
        term.advance(
            format!(
                "{}\r\n0O1Il {{}} [] () -> != ===\r\n┌──┐│ e\u{301} 漢 😀\r\nrow below\x1b[?25l",
                font.name
            )
            .as_bytes(),
        );
        let cells = term.snapshot(0).cells;
        let first = render(
            &device,
            &queue,
            &mut renderer,
            &term,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        assert_eq!(term.snapshot(0).cells, cells);
        term.advance(b"\x1b[2;1H\x1b[2Kchanged font sample");
        let second = render(
            &device,
            &queue,
            &mut renderer,
            &term,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        assert_eq!(
            &first[..512 * 4 * 32],
            &second[..512 * 4 * 32],
            "{} row bleed",
            font.name
        );
        assert_ne!(
            &first[512 * 4 * 32..512 * 4 * 56],
            &second[512 * 4 * 32..512 * 4 * 56],
            "{} has no rendered text",
            font.name
        );
        assert!(
            renderer.atlas.get('漢' as u32).is_some(),
            "{} CJK fallback",
            font.name
        );
        assert!(
            renderer.atlas.get('─' as u32).is_some(),
            "{} box drawing fallback",
            font.name
        );
        let mut ppm = b"P6\n512 256\n255\n".to_vec();
        for pixel in first.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(
            format!(
                "target/acceptance/fonts/{}.ppm",
                font.name.replace(' ', "-")
            ),
            ppm,
        )
        .unwrap();
    }
    for px in [8., 16., 40.] {
        let mut renderer = Renderer::new_with_family(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            2048,
            (px * 0.625, px * 1.375),
            8.,
            px,
            "IBM Plex Mono",
        )
        .unwrap();
        let mut term = TerminalState::new(16, 3);
        term.advance(b"sizing 0O1Il\r\nnext row\x1b[?25l");
        let frame = render(
            &device,
            &queue,
            &mut renderer,
            &term,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        assert!(
            frame
                .chunks_exact(4)
                .filter(|p| p[..3] != frame[..3])
                .count()
                > 40
        );
    }
}
fn render_scene(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    term: &TerminalState,
    format: wgpu::TextureFormat,
    scene: Option<&juna::render::overlay::Overlay>,
) -> Vec<u8> {
    render_scene_at(device, queue, renderer, term, format, scene, (512, 256))
}
fn render_scene_at(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    term: &TerminalState,
    format: wgpu::TextureFormat,
    scene: Option<&juna::render::overlay::Overlay>,
    size: (u32, u32),
) -> Vec<u8> {
    let (width, height) = size;
    assert_eq!(width % 64, 0);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("acceptance-frame"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (width * height * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    renderer.build(&term.snapshot(0), width, height);
    if let Some(scene) = scene {
        renderer.overlay(scene);
    }
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("acceptance"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.draw(device, queue, width, height, &mut pass);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let pixels = readback.slice(..).get_mapped_range().to_vec();
    readback.unmap();
    pixels
}
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    term: &TerminalState,
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    render_scene(device, queue, renderer, term, format, None)
}
#[test]
#[ignore = "real GPU and system fonts; run --test render_acceptance -- --ignored"]
fn rows_are_isolated_and_supported_text_is_visible() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut renderer = Renderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        2048,
        (20., 28.),
        8.,
        20.,
    )
    .unwrap();
    renderer
        .apply_theme(
            &juna::theme::builtins()
                .into_iter()
                .find(|t| t.id == "dracula")
                .unwrap(),
            4.5,
        )
        .unwrap();
    let mut term = TerminalState::new(24, 8);
    term.advance("Juna terminal acceptance\r\nspaces work here\r\ne\u{301} 漢 😀\r\n\x1b[31;44mWide 漢\x1b[0m\r\n\x1b[1mBold\x1b[0;3m Italic\x1b[0;4m Underline\x1b[0m\r\n\x1b[7mInverse\x1b[0m\r\n\x1b[?25l".as_bytes());
    let first = render(
        &device,
        &queue,
        &mut renderer,
        &term,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    term.advance(b"\x1b[2;1H\x1b[2Knew input line");
    let second = render(
        &device,
        &queue,
        &mut renderer,
        &term,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    assert_eq!(
        &first[..512 * 4 * 36],
        &second[..512 * 4 * 36],
        "typing in row 1 changed row 0"
    );
    assert_ne!(
        &first[512 * 4 * 36..512 * 4 * 64],
        &second[512 * 4 * 36..512 * 4 * 64]
    );
    let bg = [40u8, 42, 54];
    let row_text = &second[512 * 4 * 64..512 * 4 * 92];
    assert!(
        row_text.chunks_exact(4).filter(|p| p[..3] != bg).count() > 100,
        "cluster row has no visible glyphs"
    );
    std::fs::create_dir_all("target/acceptance").unwrap();
    let mut ppm = b"P6\n512 256\n255\n".to_vec();
    for p in second.chunks_exact(4) {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write("target/acceptance/render.ppm", ppm).unwrap();
    assert!(
        renderer.atlas.get('漢' as u32).is_some(),
        "CJK fallback unavailable"
    );
}

#[test]
#[ignore = "real GPU; checks the color conversion used by the window"]
fn palette_bytes_are_preserved_on_srgb_surfaces() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer =
            Renderer::new(&device, &queue, format, 2048, (20., 28.), 8., 20.).unwrap();
        let mut term = TerminalState::new(24, 8);
        term.advance(b"\x1b[48;2;40;42;54m \x1b[48;2;189;147;249m \x1b[?25l");
        let pixels = render(&device, &queue, &mut renderer, &term, format);
        for (x, want) in [(12, [40u8, 42, 54]), (32, [189u8, 147, 249])] {
            let i = (12 * 512 + x) * 4;
            assert_eq!(
                &pixels[i..i + 3],
                &want,
                "{format:?} changed the palette background"
            );
        }
    }
}

#[test]
#[ignore = "real GPU; captures theme and context-menu previews"]
fn theme_previews_and_ui_overlays_render_without_mutating_terminal_state() {
    use juna::app::ui::{Kind, Ui};
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut term = TerminalState::new(24, 8);
    term.advance(b"Theme preview\r\n\x1b[37;45mReadable light box\x1b[0m\r\n\x1b[?25l");
    let before = term.snapshot(0).cells;
    let mut renderer = Renderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        2048,
        (20., 28.),
        8.,
        20.,
    )
    .unwrap();
    let themes = juna::theme::builtins();
    std::fs::create_dir_all("target/acceptance/themes").unwrap();
    for theme in &themes {
        renderer.apply_theme(theme, 4.5).unwrap();
        let pixels = render(
            &device,
            &queue,
            &mut renderer,
            &term,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let wanted = juna::theme::rgb(&theme.colors.background)
            .unwrap()
            .map(|v| (v * 255.).round() as u8);
        let p = (200 * 512 + 12) * 4;
        assert_eq!(&pixels[p..p + 3], &wanted);
        let mut ppm = b"P6\n512 256\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(format!("target/acceptance/themes/{}.ppm", theme.id), ppm).unwrap();
    }
    let settings = juna::settings::Settings {
        favourite_theme_ids: vec!["catppuccin-mocha".into(), "nord".into()],
        ..Default::default()
    };
    renderer.apply_theme(&themes[0], 4.5).unwrap();
    let mut menu_themes = themes.clone();
    let mut custom = themes[0].clone();
    custom.id = "custom-capture".into();
    custom.name = "My Mocha".into();
    custom.source = None;
    menu_themes.push(custom);
    for kind in [
        Kind::Updates,
        Kind::ConfirmUpdate,
        Kind::Context,
        Kind::ConfirmClose,
        Kind::TabContext,
        Kind::Rename,
        Kind::Themes,
        Kind::Settings,
        Kind::Appearance,
        Kind::TerminalSettings,
        Kind::Tools,
        Kind::ImportExport,
        Kind::Junafetch,
        Kind::FetchProfiles,
        Kind::FetchArtwork,
        Kind::Fonts,
        Kind::Editor,
        Kind::Number,
        Kind::Custom,
        Kind::Manage,
    ] {
        let mut ui = if kind == Kind::Number {
            Ui::number(1, &settings, [8., 8.])
        } else {
            Ui::new(kind, [8., 8.])
        };
        if kind == Kind::Editor {
            ui.draft = Some(juna::theme::default_theme());
        }
        if matches!(kind, Kind::TabContext | Kind::Rename) {
            ui.target_tab = Some(1);
        }
        if matches!(kind, Kind::ConfirmClose | Kind::ConfirmUpdate) {
            ui.focus = 1;
            ui.message = if kind == Kind::ConfirmUpdate {
                "All tabs in this window will close"
            } else {
                "Closing this tab will stop its commands"
            }
            .into();
        }
        if kind == Kind::Updates {
            ui.summary = vec![
                "Installed: v0.1.1-dev12".into(),
                "Available: v0.1.1-dev13".into(),
                "Verified: v0.1.1-dev13".into(),
                "Checksum, version and source".into(),
                "verified".into(),
            ];
        }
        if kind == Kind::Rename {
            ui.draft_value = "Project shell".into();
        }
        if matches!(kind, Kind::Themes | Kind::Custom) {
            ui.expanded.insert("Catppuccin".into());
        }
        if kind == Kind::Manage {
            ui.preview_id = Some("custom-capture".into());
        }
        let rows = ui.rows(&menu_themes, &settings);
        ui.focus_row(
            if matches!(kind, Kind::ConfirmClose | Kind::ConfirmUpdate) {
                1
            } else {
                0
            },
            &rows,
        );
        let (_, visible, _) = ui.geometry(512., 256., 1., rows.len());
        ui.ensure_visible(visible, rows.len());
        let scene = ui.scene(&rows, 512., 256., 1.);
        assert!(!scene.text.is_empty());
        let pixels = render_scene(
            &device,
            &queue,
            &mut renderer,
            &term,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Some(&scene),
        );
        let mut ppm = b"P6\n512 256\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        let name = match kind {
            Kind::Updates => "updates-menu",
            Kind::ConfirmUpdate => "update-confirmation",
            Kind::ImportReview => "settings-import",
            Kind::ConfirmClose => "confirm-close-tab",
            Kind::TabContext => "tab-context",
            Kind::Rename => "rename-tab",
            Kind::Context => "context-menu",
            Kind::Themes => "theme-menu",
            Kind::Settings => "settings-menu",
            Kind::Appearance => "appearance-menu",
            Kind::TerminalSettings => "terminal-settings-menu",
            Kind::Tools => "tools-menu",
            Kind::ImportExport => "import-export-menu",
            Kind::Junafetch => "junafetch-menu",
            Kind::FetchProfiles => "junafetch-profiles",
            Kind::FetchArtwork => "junafetch-artwork",
            Kind::Fonts => "font-menu",
            Kind::Editor => "theme-editor",
            Kind::Number => "number-prompt",
            Kind::Custom => "custom-menu",
            Kind::Manage => "manage-theme",
        };
        std::fs::write(format!("target/acceptance/{name}.ppm"), ppm).unwrap();
        if !matches!(kind, Kind::Context | Kind::TabContext) {
            let scene = ui.scene(&rows, 1024., 512., 1.);
            let baseline = render_scene_at(
                &device,
                &queue,
                &mut renderer,
                &term,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                None,
                (1024, 512),
            );
            let pixels = render_scene_at(
                &device,
                &queue,
                &mut renderer,
                &term,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                Some(&scene),
                (1024, 512),
            );
            for y in 0..512 {
                let start = y * 1024 * 4;
                assert_eq!(
                    &pixels[start..start + 480 * 4],
                    &baseline[start..start + 480 * 4],
                    "preview obscured the left terminal text"
                );
            }
            let mut ppm = b"P6\n1024 512\n255\n".to_vec();
            for pixel in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(format!("target/acceptance/{name}-right.ppm"), ppm).unwrap();
        }
    }
    renderer.set_top_inset(juna::app::ui::TAB_HEIGHT);
    let labels = vec![
        "PowerShell".into(),
        "Command Prompt".into(),
        "PowerShell 7".into(),
    ];
    let scene = juna::app::ui::tab_scene(&labels, 1, &[true, false, false], true, 1024., 1.);
    let pixels = render_scene_at(
        &device,
        &queue,
        &mut renderer,
        &term,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some(&scene),
        (1024, 512),
    );
    let mut ppm = b"P6\n1024 512\n255\n".to_vec();
    for pixel in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write("target/acceptance/tabs.ppm", ppm).unwrap();
    let mut numbers = Renderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        2048,
        (10., 22.),
        8.,
        16.,
    )
    .unwrap();
    let inset = juna::app::gutter::width(1024., 1., true);
    numbers.set_left_inset(inset);
    numbers.set_top_inset(juna::app::ui::TAB_HEIGHT);
    let mut scene = juna::app::ui::tab_scene(&labels, 1, &[true, false, false], true, 1024., 1.);
    scene
        .text
        .extend(juna::app::gutter::scene(inset, 1., 171, 8, [0.8, 0.8, 0.9, 1.]).text);
    let pixels = render_scene_at(
        &device,
        &queue,
        &mut numbers,
        &term,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some(&scene),
        (1024, 512),
    );
    let mut ppm = b"P6\n1024 512\n255\n".to_vec();
    for pixel in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write("target/acceptance/line-numbers.ppm", ppm).unwrap();
    assert_eq!(term.snapshot(0).cells, before);
}
