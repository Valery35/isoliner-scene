// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Отрисовка сцены на wgpu.
//!
//! Кадр рисуется в свою текстуру ([`Target`]), а не прямо в окно:
//! ту же текстуру показывает окно (egui), с неё же снимается кадр
//! для снимка и видео, и она же рисуется без окна в тестах.
//!
//! Порядок кадра: непрозрачные тела с записью глубины, линии,
//! прозрачные тела от дальних к ближним без записи глубины.
//! Мировые координаты в видеокарту не попадают: вершины выгружаются
//! сдвинутыми на центр сцены ([`scene_core::Frame`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use bytemuck::{Pod, Zeroable};
use glam::{DMat3, DVec3};
use wgpu::util::DeviceExt;

use scene_core::camera::{View, eye};
use scene_core::{Camera, Frame};
use scene_format::{Indices, Part};

/// Формат кадра. Без sRGB: окно модуля (OpenGL pyqtgraph) считает
/// свет, фон и прозрачность прямо в числах экрана, и кадр здесь
/// держит те же числа. egui показывает такую текстуру как есть,
/// а текстуру sRGB на WebGPU показывал затемнённой.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Сглаживание краёв: отсчётов на пиксель.
pub const SAMPLES: u32 = 4;
/// Цвет части без своего цвета.
pub const DEFAULT_COLOR: [f32; 4] = [0.70, 0.72, 0.75, 1.0];
/// Фон окна модуля с градиентом (`_bg_apply`): верх и низ, sRGB.
/// Сверху холоднее, снизу светлее, как небо и земля.
pub const BG_GRADIENT: Background = Background {
    top: [0.78, 0.84, 0.92],
    bottom: [0.98, 0.98, 0.96],
};
/// Ровный фон окна модуля без градиента: 250, 250, 248.
pub const BG_PLAIN: Background = Background {
    top: [250.0 / 255.0, 250.0 / 255.0, 248.0 / 255.0],
    bottom: [250.0 / 255.0, 250.0 / 255.0, 248.0 / 255.0],
};

/// Фон тёмной темы с градиентом: ночное небо сверху, сумерки снизу.
pub const BG_GRADIENT_DARK: Background = Background {
    top: [0.11, 0.13, 0.17],
    bottom: [0.25, 0.27, 0.29],
};
/// Ровный фон тёмной темы.
pub const BG_PLAIN_DARK: Background = Background {
    top: [0.16, 0.17, 0.18],
    bottom: [0.16, 0.17, 0.18],
};

/// Три источника мягкого света окна модуля (`lights.py`): направление
/// в координатах камеры и вес. Основной рисует форму, второй светит
/// с другой стороны вполсилы, третий снизу подсвечивает.
const LIGHTS: [([f64; 3], f32); 3] = [
    ([1.0, -1.0, -1.0], 0.55),
    ([-1.0, 0.5, -0.8], 0.28),
    ([0.2, 1.0, -0.5], 0.17),
];

/// Фон кадра: цвет верха и низа, sRGB в долях единицы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    pub top: [f32; 3],
    pub bottom: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 4],
}

/// Точка-значок: положение, цвет, размер в пикселях экрана.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointMark {
    pos: [f32; 3],
    color: [f32; 4],
    size: f32,
}

/// Размер значка точки без `_SIZE`, пиксели.
pub const DEFAULT_POINT_SIZE: f32 = 8.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    lights: [[f32; 4]; 3],
    params: [f32; 4],
    bg_top: [f32; 4],
    bg_bottom: [f32; 4],
    /// ширина и высота кадра в пикселях, пикселей на точку экрана
    screen: [f32; 4],
}

/// Часть сцены в видеокарте.
pub struct GpuPart {
    pub name: String,
    pub lines: bool,
    /// Точки-значки: квадрат на точку, круг в шейдере.
    pub points: bool,
    pub transparent: bool,
    pub visible: bool,
    /// Середина части в координатах сцены без преувеличения.
    pub center: [f64; 3],
    vbuf: wgpu::Buffer,
    /// У точек номеров нет: точка - один экземпляр квадрата.
    ibuf: Option<wgpu::Buffer>,
    count: u32,
}

/// Сцена в видеокарте.
pub struct GpuScene {
    pub parts: Vec<GpuPart>,
    /// Центр, на который сдвинуты вершины (без преувеличения).
    pub frame: Frame,
}

impl GpuScene {
    /// Выгрузить части. Вершины сдвигаются на центр `frame`
    /// в double и только потом сводятся к float32.
    pub fn upload(device: &wgpu::Device, parts: &[Part], frame: Frame) -> Self {
        let out = parts
            .iter()
            .filter_map(|p| GpuPart::upload(device, p, frame))
            .collect();
        Self { parts: out, frame }
    }
}

impl GpuPart {
    /// Выгрузить одну часть. Пустая часть - None.
    pub fn upload(device: &wgpu::Device, part: &Part, frame: Frame) -> Option<Self> {
        if part.verts.is_empty() || part.indices.is_empty() {
            return None;
        }
        if let Indices::Points(idx) = &part.indices {
            return Some(upload_points(device, part, idx, frame));
        }
        let mut sum = [0.0f64; 3];
        let verts: Vec<Vertex> = part
            .verts
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = [v[0] - frame.cx, v[1] - frame.cy, v[2] - frame.cz];
                for k in 0..3 {
                    sum[k] += p[k];
                }
                let color = part
                    .colors
                    .as_ref()
                    .and_then(|c| c.get(i).copied())
                    .unwrap_or(DEFAULT_COLOR);
                Vertex {
                    pos: [p[0] as f32, p[1] as f32, p[2] as f32],
                    color,
                }
            })
            .collect();
        let n = verts.len() as f64;
        let (flat, lines): (Vec<u32>, bool) = match &part.indices {
            Indices::Triangles(t) => (t.iter().flatten().copied().collect(), false),
            Indices::Lines(l) => (l.iter().flatten().copied().collect(), true),
            Indices::Points(_) => return None,
        };
        let transparent = part
            .colors
            .as_ref()
            .is_some_and(|c| c.iter().any(|c| c[3] < 0.999));
        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("вершины части"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("номера вершин части"),
            contents: bytemuck::cast_slice(&flat),
            usage: wgpu::BufferUsages::INDEX,
        });
        Some(GpuPart {
            name: part.name.clone(),
            lines,
            points: false,
            transparent,
            visible: true,
            center: [sum[0] / n, sum[1] / n, sum[2] / n],
            vbuf,
            ibuf: Some(ibuf),
            count: flat.len() as u32,
        })
    }

    /// Рисовать нечего: от части после обрезки ничего не осталось.
    /// Часть остаётся в списке, номера частей не сдвигаются.
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

/// Точки части в буфер экземпляров. Номер точки за краем вершин
/// пропускается: читатель пакета их уже проверил, здесь - на всякий
/// случай без паники.
fn upload_points(device: &wgpu::Device, part: &Part, idx: &[u32], frame: Frame) -> GpuPart {
    let mut sum = [0.0f64; 3];
    let marks: Vec<PointMark> = idx
        .iter()
        .filter_map(|&i| {
            let v = part.verts.get(i as usize)?;
            let p = [v[0] - frame.cx, v[1] - frame.cy, v[2] - frame.cz];
            for k in 0..3 {
                sum[k] += p[k];
            }
            let i = i as usize;
            Some(PointMark {
                pos: [p[0] as f32, p[1] as f32, p[2] as f32],
                color: part
                    .colors
                    .as_ref()
                    .and_then(|c| c.get(i).copied())
                    .unwrap_or(DEFAULT_COLOR),
                size: part
                    .sizes
                    .as_ref()
                    .and_then(|s| s.get(i).copied())
                    .filter(|s| s.is_finite() && *s > 0.0)
                    .unwrap_or(DEFAULT_POINT_SIZE),
            })
        })
        .collect();
    let n = marks.len().max(1) as f64;
    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("точки части"),
        contents: bytemuck::cast_slice(&marks),
        usage: wgpu::BufferUsages::VERTEX,
    });
    GpuPart {
        name: part.name.clone(),
        lines: false,
        points: true,
        transparent: marks.iter().any(|m| m.color[3] < 0.999),
        visible: true,
        center: [sum[0] / n, sum[1] / n, sum[2] / n],
        vbuf,
        ibuf: None,
        count: marks.len() as u32,
    }
}

/// Текстура кадра с глубиной и сглаживанием.
pub struct Target {
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    msaa_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl Target {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let make = |label, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = make(
            "кадр",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa = make(
            "кадр со сглаживанием",
            COLOR_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let depth = make(
            "глубина",
            DEPTH_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        Self {
            width,
            height,
            color_view: color.create_view(&wgpu::TextureViewDescriptor::default()),
            color,
            msaa_view: msaa.create_view(&wgpu::TextureViewDescriptor::default()),
            depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }
}

/// Настройки кадра.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Камера в координатах просмотра (сдвиг и преувеличение кадра сцены).
    pub camera: Camera,
    /// Преувеличение по вертикали.
    pub vex: f64,
    /// Общая непрозрачность тел, 0..1.
    pub opacity: f32,
    /// Фон кадра.
    pub background: Background,
    /// Пикселей кадра на пиксель значка: размер точки в пакете задан
    /// в пикселях экрана окна QGIS.
    pub pixels_per_point: f32,
}

/// Конвейеры и общие буферы.
pub struct Renderer {
    background: wgpu::RenderPipeline,
    points: wgpu::RenderPipeline,
    mesh_solid: wgpu::RenderPipeline,
    mesh_blend: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("сцена"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("кадр"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("кадр"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("кадр"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("сцена"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        })];
        let make = |label: &str, fs: &str, topo, blend: bool, write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: topo,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // фон: без вершин и без глубины, рисуется первым
        let background = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("фон"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_bg"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_bg"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // точки: квадрат из шести вершин на экземпляр
        let pattrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32];
        let points = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("точки"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_point"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<PointMark>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &pattrs,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_point"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            background,
            points,
            mesh_solid: make(
                "тела",
                "fs_mesh",
                wgpu::PrimitiveTopology::TriangleList,
                false,
                true,
            ),
            mesh_blend: make(
                "тела прозрачные",
                "fs_mesh",
                wgpu::PrimitiveTopology::TriangleList,
                true,
                false,
            ),
            lines: make(
                "линии",
                "fs_line",
                wgpu::PrimitiveTopology::LineList,
                true,
                true,
            ),
            uniforms,
            bind,
        }
    }

    /// Нарисовать кадр в `target`.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &Target,
        scene: &GpuScene,
        look: &Look,
    ) {
        let cam = look.camera;
        let view = View::new(&cam, f64::from(target.width), f64::from(target.height));
        let e = DVec3::from(eye(&cam));
        // Свет задан в координатах камеры, как в окне модуля, и не
        // «залипает» на модели при вращении. В шейдер он идёт
        // в координатах просмотра: поворот вида, обращённый.
        let to_view = DMat3::from_mat4(view.view).transpose();
        let lights = LIGHTS.map(|(d, w)| {
            let l = (to_view * DVec3::from(d)).normalize_or_zero();
            [l.x as f32, l.y as f32, l.z as f32, w]
        });
        let bg = look.background;
        let u = Uniforms {
            view_proj: view.view_proj().as_mat4().to_cols_array_2d(),
            lights,
            params: [look.vex as f32, look.opacity.clamp(0.0, 1.0), 0.0, 0.0],
            bg_top: [bg.top[0], bg.top[1], bg.top[2], 1.0],
            bg_bottom: [bg.bottom[0], bg.bottom[1], bg.bottom[2], 1.0],
            screen: [
                target.width as f32,
                target.height as f32,
                look.pixels_per_point.max(0.1),
                0.0,
            ],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));

        // прозрачные - от дальних к ближним
        let vex = look.vex;
        let dist = |p: &GpuPart| {
            let q = DVec3::new(p.center[0], p.center[1], p.center[2] * vex);
            (q - e).length()
        };
        let mut soft: Vec<&GpuPart> = scene
            .parts
            .iter()
            .filter(|p| {
                p.visible && !p.lines && !p.points && (p.transparent || look.opacity < 0.999)
            })
            .collect();
        soft.sort_by(|a, b| dist(b).total_cmp(&dist(a)));

        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("кадр"),
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("сцена"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&target.color_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_pipeline(&self.background);
            pass.draw(0..3, 0..1);
            let draw = |pass: &mut wgpu::RenderPass<'_>, p: &GpuPart| {
                pass.set_vertex_buffer(0, p.vbuf.slice(..));
                match &p.ibuf {
                    Some(ib) => {
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..p.count, 0, 0..1);
                    }
                    None => pass.draw(0..6, 0..p.count),
                }
            };
            pass.set_pipeline(&self.mesh_solid);
            for p in scene.parts.iter().filter(|p| {
                p.visible && !p.lines && !p.points && !p.transparent && look.opacity >= 0.999
            }) {
                draw(&mut pass, p);
            }
            // точки - после тел и до прозрачных: значок пробы виден
            // сквозь полупрозрачный пласт, как в окне модуля
            pass.set_pipeline(&self.points);
            for p in scene.parts.iter().filter(|p| p.visible && p.points) {
                draw(&mut pass, p);
            }
            pass.set_pipeline(&self.lines);
            for p in scene.parts.iter().filter(|p| p.visible && p.lines) {
                draw(&mut pass, p);
            }
            pass.set_pipeline(&self.mesh_blend);
            for p in soft {
                draw(&mut pass, p);
            }
        }
        queue.submit(Some(enc.finish()));
    }
}

/// Снимок кадра, который ещё читается из видеокарты. В браузере
/// ждать видеокарту нельзя: окно спрашивает [`Readback::poll`]
/// на каждом кадре, пока снимок не будет готов.
pub struct Readback {
    buf: wgpu::Buffer,
    width: u32,
    height: u32,
    padded: u32,
    state: std::sync::Arc<std::sync::Mutex<Option<Result<(), String>>>>,
}

impl Readback {
    /// Ширина и высота снимка в пикселях.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Готовый снимок в RGBA, строки сверху вниз, или None, если
    /// видеокарта ещё не отдала его.
    pub fn poll(&self, device: &wgpu::Device) -> Option<Result<Vec<u8>, String>> {
        // на диске отображение буфера двигает poll, в браузере - сам браузер
        if let Err(e) = device.poll(wgpu::PollType::Poll) {
            return Some(Err(e.to_string()));
        }
        let done = self.state.lock().ok()?.take()?;
        Some(done.and_then(|()| self.collect()))
    }

    fn collect(&self) -> Result<Vec<u8>, String> {
        let row = self.width as usize * 4;
        let data = self
            .buf
            .slice(..)
            .get_mapped_range()
            .map_err(|e| format!("{e:?}"))?;
        let mut out = Vec::with_capacity(row * self.height as usize);
        for y in 0..self.height as usize {
            let s = y * self.padded as usize;
            out.extend_from_slice(data.get(s..s + row).ok_or("снимок обрезан")?);
        }
        drop(data);
        self.buf.unmap();
        Ok(out)
    }
}

/// Начать снимок кадра `target`.
pub fn start_readback(device: &wgpu::Device, queue: &wgpu::Queue, target: &Target) -> Readback {
    let row = target.width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("снимок"),
        size: u64::from(padded) * u64::from(target.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("снимок"),
    });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(target.height),
            },
        },
        wgpu::Extent3d {
            width: target.width,
            height: target.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(enc.finish()));
    let state = std::sync::Arc::new(std::sync::Mutex::new(None));
    let to = state.clone();
    buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        if let Ok(mut s) = to.lock() {
            *s = Some(r.map_err(|e| e.to_string()));
        }
    });
    Readback {
        buf,
        width: target.width,
        height: target.height,
        padded,
        state,
    }
}

/// Снять кадр в RGBA, строки сверху вниз, с ожиданием видеокарты.
/// Только на диске: в браузере ждать нельзя, там [`start_readback`].
pub fn read_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &Target,
) -> Result<Vec<u8>, String> {
    let rb = start_readback(device, queue, target);
    // не дольше минуты: занятая видеокарта даёт ошибку, а не зависание
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(60)),
        })
        .map_err(|e| e.to_string())?;
    rb.poll(device)
        .unwrap_or_else(|| Err("видеокарта не отдала снимок".to_string()))
}

/// Камера пакета в координатах просмотра для кадра `frame`.
pub fn view_camera(frame: &Frame, cam: &Camera) -> Camera {
    frame.camera_to_view(cam)
}

/// Видеокарта без окна: для тестов, снимков и видео.
pub fn headless() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|e| e.to_string())?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| e.to_string())
}
