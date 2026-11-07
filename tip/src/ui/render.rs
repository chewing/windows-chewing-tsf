// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

//! Generic renderer for a 2D [`TaffyTree`] of text nodes and panels.

use std::collections::HashMap;

use anyhow::Result;
use taffy::{
    compute_leaf_layout,
    geometry::{Point, Size},
    prelude::*,
    tree::{LayoutInput, LayoutOutput, NodeId, TaffyTree},
};
use windows::Win32::Graphics::{
    Direct2D::{
        CLSID_D2D1GaussianBlur,
        Common::{D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_COMPOSITE_MODE_SOURCE_OVER},
        D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
        D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT, ID2D1DeviceContext,
        ID2D1SolidColorBrush,
    },
    DirectWrite::{
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
        IDWriteFactory1, IDWriteTextFormat,
    },
};
use windows_core::{HSTRING, w};
use windows_numerics::Vector2;

const LOCALE: windows_core::PCWSTR = w!("zh-TW");

pub(crate) const MESSAGE_BOX_BLUR: f32 = 3.0;
const MESSAGE_BOX_BLEED: f32 = 25.0;
const MESSAGE_BOX_RADIUS: f32 = 8.0;
const MESSAGE_BOX_BORDER: f32 = 0.5;

#[derive(Clone, Debug)]
pub(crate) struct TextStyle {
    pub font_family: String,
    pub font_size: f32,
    pub color: D2D1_COLOR_F,
}

impl TextStyle {
    pub(crate) fn new(font_family: impl Into<String>, font_size: f32, color: D2D1_COLOR_F) -> Self {
        Self {
            font_family: font_family.into(),
            font_size,
            color,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Text {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shadow {
    pub color: D2D1_COLOR_F,
    pub blur_radius: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct Panel {
    pub fill: D2D1_COLOR_F,
    pub border: D2D1_COLOR_F,
    pub border_width: f32,
    pub corner_radius: f32,
    pub shadow: Option<Shadow>,
}

impl Panel {
    pub(crate) fn message_box(bg: D2D1_COLOR_F, border: D2D1_COLOR_F) -> Self {
        Self {
            fill: bg,
            border,
            border_width: MESSAGE_BOX_BORDER,
            corner_radius: MESSAGE_BOX_RADIUS,
            shadow: Some(Shadow {
                color: D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.1 * bg.a,
                },
                blur_radius: MESSAGE_BOX_BLUR,
            }),
        }
    }

    pub(crate) fn highlight(fill: D2D1_COLOR_F) -> Self {
        Self {
            fill,
            border: fill,
            border_width: 0.0,
            corner_radius: 0.0,
            shadow: None,
        }
    }

    pub(crate) fn bleed(&self) -> f32 {
        self.shadow
            .as_ref()
            .map_or(0.0, |s| MESSAGE_BOX_BLEED + s.blur_radius)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Node {
    pub panel: Option<Panel>,
    pub text: Option<Text>,
}

pub(crate) type UiTree = TaffyTree<Node>;

pub(crate) fn text_node(
    tree: &mut UiTree,
    style: Style,
    text: impl Into<String>,
    text_style: TextStyle,
) -> Result<NodeId> {
    panel_text(tree, style, text, text_style, None)
}

pub(crate) fn panel_text(
    tree: &mut UiTree,
    style: Style,
    text: impl Into<String>,
    text_style: TextStyle,
    panel: Option<Panel>,
) -> Result<NodeId> {
    Ok(tree.new_leaf_with_context(
        style,
        Node {
            panel,
            text: Some(Text {
                text: text.into(),
                style: text_style,
            }),
        },
    )?)
}

pub(crate) fn panel_node(
    tree: &mut UiTree,
    style: Style,
    children: &[NodeId],
    panel: Option<Panel>,
) -> Result<NodeId> {
    let id = tree.new_with_children(style, children)?;
    if panel.is_some() {
        tree.set_node_context(id, Some(Node { panel, text: None }))?;
    }
    Ok(id)
}

/// DirectWrite text measurement with a small per-instance format cache.
pub(crate) struct TextMeasurer {
    factory: IDWriteFactory1,
    formats: HashMap<(String, u32), IDWriteTextFormat>,
}

impl TextMeasurer {
    pub(crate) fn new(factory: &IDWriteFactory1) -> Self {
        Self {
            factory: factory.clone(),
            formats: HashMap::new(),
        }
    }

    pub(crate) fn format(&mut self, style: &TextStyle) -> Result<IDWriteTextFormat> {
        let key = (style.font_family.clone(), style.font_size.to_bits());
        if let Some(format) = self.formats.get(&key) {
            return Ok(format.clone());
        }
        let family = HSTRING::from(style.font_family.as_str());
        let format = unsafe {
            let format = self.factory.CreateTextFormat(
                &family,
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.font_size,
                LOCALE,
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format
        };
        self.formats.insert(key, format.clone());
        Ok(format)
    }

    pub(crate) fn measure_text(&mut self, text: &str, style: &TextStyle) -> Result<(f32, f32)> {
        let format = self.format(style)?;
        let text = HSTRING::from(text);
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            self.factory
                .CreateTextLayout(&text, &format, f32::MAX, f32::MAX)?
                .GetMetrics(&mut metrics)?;
        }
        Ok((
            metrics.widthIncludingTrailingWhitespace.ceil(),
            metrics.height.ceil(),
        ))
    }
}

/// Measures every text leaf and computes the layout. Returns the root size.
pub(crate) fn compute_layout(
    tree: &mut UiTree,
    root: NodeId,
    measurer: &mut TextMeasurer,
) -> Result<Size<f32>> {
    let mut failure: Option<anyhow::Error> = None;

    let zero = |inputs: LayoutInput, style: &Style| -> LayoutOutput {
        compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO)
    };

    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
        |inputs, _node_id, context, style| {
            let Some(text) = context.and_then(|node| node.text.as_ref()) else {
                return zero(inputs, style);
            };
            match measurer.measure_text(&text.text, &text.style) {
                Ok((width, height)) => compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, _available| Size {
                        width: known.width.unwrap_or(width),
                        height: known.height.unwrap_or(height),
                    },
                ),
                Err(error) => {
                    if failure.is_none() {
                        failure = Some(error);
                    }
                    zero(inputs, style)
                }
            }
        },
    )?;

    if let Some(error) = failure {
        return Err(error);
    }

    Ok(tree.layout(root)?.size)
}

#[derive(Default)]
struct BrushCache {
    brushes: HashMap<[u32; 4], ID2D1SolidColorBrush>,
}

impl BrushCache {
    fn get(
        &mut self,
        dc: &ID2D1DeviceContext,
        color: &D2D1_COLOR_F,
    ) -> Result<ID2D1SolidColorBrush> {
        let key = [
            color.r.to_bits(),
            color.g.to_bits(),
            color.b.to_bits(),
            color.a.to_bits(),
        ];
        if let Some(brush) = self.brushes.get(&key) {
            return Ok(brush.clone());
        }
        let brush = unsafe { dc.CreateSolidColorBrush(color, None)? };
        self.brushes.insert(key, brush.clone());
        Ok(brush)
    }
}

/// Draws every panel and text run in the tree, offset by `origin` (in DIPs).
pub(crate) fn paint(
    tree: &UiTree,
    root: NodeId,
    dc: &ID2D1DeviceContext,
    measurer: &mut TextMeasurer,
    origin: Point<f32>,
) -> Result<()> {
    let mut brushes = BrushCache::default();
    paint_node(tree, root, dc, measurer, &mut brushes, origin)
}

fn paint_node(
    tree: &UiTree,
    node: NodeId,
    dc: &ID2D1DeviceContext,
    measurer: &mut TextMeasurer,
    brushes: &mut BrushCache,
    origin: Point<f32>,
) -> Result<()> {
    let layout = tree.layout(node)?;
    let origin = Point {
        x: origin.x + layout.location.x,
        y: origin.y + layout.location.y,
    };
    let rect = D2D_RECT_F {
        left: origin.x,
        top: origin.y,
        right: origin.x + layout.size.width,
        bottom: origin.y + layout.size.height,
    };

    if let Some(context) = tree.get_node_context(node) {
        if let Some(panel) = &context.panel {
            paint_panel(dc, brushes, &rect, panel)?;
        }

        if let Some(text) = &context.text {
            let format = measurer.format(&text.style)?;
            let brush = brushes.get(dc, &text.style.color)?;
            let text = HSTRING::from(text.text.as_str());
            let content = D2D_RECT_F {
                left: rect.left + layout.border.left + layout.padding.left,
                top: rect.top + layout.border.top + layout.padding.top,
                right: rect.right - layout.border.right - layout.padding.right,
                bottom: rect.bottom - layout.border.bottom - layout.padding.bottom,
            };
            unsafe {
                dc.DrawText(
                    &text,
                    &format,
                    &content,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    for child in tree.children(node)? {
        paint_node(tree, child, dc, measurer, brushes, origin)?;
    }
    Ok(())
}

fn paint_panel(
    dc: &ID2D1DeviceContext,
    brushes: &mut BrushCache,
    rect: &D2D_RECT_F,
    panel: &Panel,
) -> Result<()> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    unsafe {
        if let Some(shadow) = &panel.shadow {
            let desired_size = D2D_SIZE_F {
                width: width + shadow.blur_radius * 2.0,
                height: height + shadow.blur_radius * 2.0,
            };
            let shadow_target = dc.CreateCompatibleRenderTarget(
                Some(&desired_size),
                None,
                None,
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            )?;
            shadow_target.BeginDraw();
            shadow_target.Clear(None);
            let shadow_brush = shadow_target.CreateSolidColorBrush(&shadow.color, None)?;
            shadow_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: shadow.blur_radius,
                        top: shadow.blur_radius,
                        right: width + shadow.blur_radius,
                        bottom: height + shadow.blur_radius,
                    },
                    radiusX: panel.corner_radius,
                    radiusY: panel.corner_radius,
                },
                &shadow_brush,
            );
            shadow_target.EndDraw(None, None)?;

            let bitmap = shadow_target.GetBitmap()?;
            let blur = dc.CreateEffect(&CLSID_D2D1GaussianBlur)?;
            blur.SetInput(0, &bitmap, false);
            dc.DrawImage(
                &blur.GetOutput()?,
                Some(&Vector2 {
                    X: rect.left,
                    Y: rect.top,
                }),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
        }

        let brush = brushes.get(dc, &panel.fill)?;
        if panel.corner_radius > 0.0 {
            dc.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: *rect,
                    radiusX: panel.corner_radius,
                    radiusY: panel.corner_radius,
                },
                &brush,
            );
        } else {
            dc.FillRectangle(rect, &brush);
        }

        if panel.border_width > 0.0 {
            let brush = brushes.get(dc, &panel.border)?;
            if panel.corner_radius > 0.0 {
                dc.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: *rect,
                        radiusX: panel.corner_radius,
                        radiusY: panel.corner_radius,
                    },
                    &brush,
                    panel.border_width,
                    None,
                );
            } else {
                dc.DrawRectangle(rect, &brush, panel.border_width, None);
            }
        }
    }
    Ok(())
}

fn hw_px(dip: f32, dpi: f32) -> f32 {
    (dip * dpi / 96.0).round()
}

/// Maps between the visible panel and the physical window frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameGeometry {
    /// Shadow bleed space per side in DIP.
    bleed: f32,
    /// Whole-tree layout size in DIP.
    size: Size<f32>,
}

impl FrameGeometry {
    pub(crate) fn new(bleed: f32, size: Size<f32>) -> Self {
        Self { bleed, size }
    }

    /// Panel size, DIP.
    pub(crate) fn panel_size(&self) -> Size<f32> {
        Size {
            width: self.size.width - 2.0 * self.bleed,
            height: self.size.height - 2.0 * self.bleed,
        }
    }

    /// Physical size for the window and swapchain.
    pub(crate) fn window_size(&self, dpi: f32) -> (f32, f32) {
        let bleed = hw_px(self.bleed, dpi).max(0.0);
        let scale = dpi / 96.0;
        let panel = self.panel_size();
        (
            (panel.width * scale).ceil().max(0.0) + 2.0 * bleed,
            (panel.height * scale).ceil().max(0.0) + 2.0 * bleed,
        )
    }

    pub(crate) fn window_origin(&self, x: i32, y: i32, dpi: f32) -> (i32, i32) {
        let bleed = hw_px(self.bleed, dpi);
        (x - bleed.ceil() as i32, y - bleed.ceil() as i32)
    }
}
