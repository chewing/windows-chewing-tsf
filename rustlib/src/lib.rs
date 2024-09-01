#[cxx::bridge]
mod ffi {
    struct RectF {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    }
    struct Patch {
        source: RectF,
        target: RectF,
    }

    extern "Rust" {
        type NinePatchDrawable;

        fn nine_patch_uninit() -> Box<NinePatchDrawable>;

        fn make_nine_patch(
            bitmap: &[u8],
            stride: usize,
            width: usize,
            height: usize,
        ) -> Box<NinePatchDrawable>;

        fn nine_patch_margin(nine_patch: &Box<NinePatchDrawable>) -> f32;

        fn nine_patch_scale_to(
            nine_patch: &Box<NinePatchDrawable>,
            width: usize,
            height: usize,
        ) -> Vec<Patch>;
    }
}

use ffi::{Patch, RectF};
use log::debug;
use nine_patch_drawable::{PatchKind, Section};

pub struct NinePatchDrawable(nine_patch_drawable::NinePatchDrawable);

pub fn nine_patch_uninit() -> Box<NinePatchDrawable> {
    win_dbg_logger::init();
    win_dbg_logger::rust_win_dbg_logger_init_debug();
    make_nine_patch(&[], 0, 0, 0)
}

pub fn make_nine_patch(
    bitmap: &[u8],
    stride: usize,
    width: usize,
    height: usize,
) -> Box<NinePatchDrawable> {
    debug!("{:?} {:?} {:?} {:?}", bitmap, stride, width, height);
    let drawable = nine_patch_drawable::NinePatchDrawable::new(bitmap, stride, width, height)
        .unwrap_or(nine_patch_drawable::NinePatchDrawable {
            width,
            height,
            h_sections: vec![Section {
                start: 1.0,
                len: width as f32 - 1.0,
                kind: PatchKind::Stretching,
            }],
            v_sections: vec![Section {
                start: 1.0,
                len: width as f32 - 1.0,
                kind: PatchKind::Stretching,
            }],
            margin_left: 0.0,
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
        });
    Box::new(NinePatchDrawable(drawable))
}

pub fn nine_patch_margin(nine_patch: &Box<NinePatchDrawable>) -> f32 {
    nine_patch.0.margin_left
}

pub fn nine_patch_scale_to(
    nine_patch: &Box<NinePatchDrawable>,
    width: usize,
    height: usize,
) -> Vec<Patch> {
    nine_patch
        .0
        .scale_to(width, height)
        .into_iter()
        .map(|p| Patch {
            source: RectF {
                left: p.source.left,
                top: p.source.top,
                right: p.source.right,
                bottom: p.source.bottom,
            },
            target: RectF {
                left: p.target.left,
                top: p.target.top,
                right: p.target.right,
                bottom: p.target.bottom,
            },
        })
        .collect()
}
