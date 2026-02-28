// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur,
    Common::{D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_COMPOSITE_MODE_SOURCE_OVER},
    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT,
    ID2D1DeviceContext,
};
use windows_numerics::Vector2;

pub(super) fn draw_message_box(
    dc: &ID2D1DeviceContext,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    bg_color: D2D1_COLOR_F,
    border_color: D2D1_COLOR_F,
) -> Result<()> {
    let blur_radius = 3.0;
    let corner_radius = 8.0;

    unsafe {
        let desired_size = D2D_SIZE_F {
            width: width + blur_radius * 2.0,
            height: height + blur_radius * 2.0,
        };
        let shadow_render_target = dc.CreateCompatibleRenderTarget(
            Some(&desired_size),
            None,
            None,
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )?;
        shadow_render_target.BeginDraw();
        shadow_render_target.Clear(None);

        let shadow_brush = shadow_render_target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.1 * bg_color.a,
            },
            None,
        )?;
        let rounded_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            radiusX: corner_radius,
            radiusY: corner_radius,
        };
        let shadow_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: blur_radius,
                top: blur_radius,
                right: width + blur_radius,
                bottom: height + blur_radius,
            },
            radiusX: corner_radius,
            radiusY: corner_radius,
        };
        shadow_render_target.FillRoundedRectangle(&shadow_rect, &shadow_brush);
        shadow_render_target.EndDraw(None, None)?;

        let shadow_bitmap = shadow_render_target.GetBitmap()?;
        let gaussian_blur_effect = dc.CreateEffect(&CLSID_D2D1GaussianBlur)?;
        gaussian_blur_effect.SetInput(0, &shadow_bitmap, false);
        let blur_output = gaussian_blur_effect.GetOutput()?;
        dc.DrawImage(
            &blur_output,
            Some(&Vector2 { X: left, Y: top }),
            None,
            D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_COMPOSITE_MODE_SOURCE_OVER,
        );
        let background_brush = dc.CreateSolidColorBrush(&bg_color, None)?;
        let border_brush = dc.CreateSolidColorBrush(&border_color, None)?;
        dc.FillRoundedRectangle(&rounded_rect, &background_brush);
        dc.DrawRoundedRectangle(&rounded_rect, &border_brush, 0.5, None);
    }
    Ok(())
}
