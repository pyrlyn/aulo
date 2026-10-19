//! T9.15: 0–999 coordinates round-trip, and a 2× capture names the same point.

use aulo_screen::{Frame, MODEL_MAX_EDGE, Norm, Pixel, ScreenError, Size, bucket, norm, pixel};

#[test]
fn corners_and_center_round_trip() {
    let size = Size {
        width: 2000,
        height: 1000,
    };
    for (x, y) in [(0, 0), (1999, 999), (1000, 500), (1, 1)] {
        let at = Pixel { x, y };
        let back = pixel(norm(at, size), size);
        assert!(
            (i64::from(back.x) - i64::from(x)).unsigned_abs() <= u64::from(bucket(size.width)),
            "x {x} -> {} -> {}",
            norm(at, size).x,
            back.x
        );
        assert!(
            (i64::from(back.y) - i64::from(y)).unsigned_abs() <= u64::from(bucket(size.height)),
            "y {y} -> {} -> {}",
            norm(at, size).y,
            back.y
        );
    }
    assert_eq!(norm(Pixel { x: 0, y: 0 }, size), Norm { x: 0, y: 0 });
    assert_eq!(
        norm(Pixel { x: 1999, y: 999 }, size),
        Norm { x: 999, y: 999 }
    );
}

#[test]
fn a_retina_capture_names_the_same_point_as_one_x() {
    let logical = Size {
        width: 1440,
        height: 900,
    };
    let retina = Size {
        width: logical.width * 2,
        height: logical.height * 2,
    };
    let on_logical = norm(Pixel { x: 720, y: 450 }, logical);
    let on_retina = norm(Pixel { x: 1440, y: 900 }, retina);
    assert_eq!(on_logical, on_retina);
    assert_eq!(on_logical, Norm { x: 500, y: 500 });
    let back = pixel(on_retina, retina);
    assert!((i64::from(back.x) - 1440).unsigned_abs() <= u64::from(bucket(retina.width)));
    assert!((i64::from(back.y) - 900).unsigned_abs() <= u64::from(bucket(retina.height)));
}

#[test]
fn a_grid_stays_inside_one_bucket() {
    let size = Size {
        width: 2560,
        height: 1440,
    };
    let mut x = 0;
    while x < size.width {
        let mut y = 0;
        while y < size.height {
            let at = Pixel { x, y };
            let back = pixel(norm(at, size), size);
            assert!(
                (i64::from(back.x) - i64::from(x)).unsigned_abs() <= u64::from(bucket(size.width))
            );
            assert!(
                (i64::from(back.y) - i64::from(y)).unsigned_abs() <= u64::from(bucket(size.height))
            );
            y += 80;
        }
        x += 80;
    }
}

#[test]
fn downscale_keeps_the_aspect_and_the_norm() {
    let screen = Size {
        width: 2000,
        height: 1000,
    };
    let rgba = solid(screen, [10, 20, 30, 255]);
    let frame = Frame::downscale(screen, &rgba).expect("frame");
    assert_eq!(frame.model.width.max(frame.model.height), MODEL_MAX_EDGE);
    assert_eq!(
        frame.model,
        Size {
            width: 1280,
            height: 640
        }
    );
    assert_eq!(
        frame.rgba.len(),
        usize::try_from(u64::from(frame.model.width) * u64::from(frame.model.height) * 4)
            .expect("len")
    );
    assert_eq!(&frame.rgba[..4], &[10, 20, 30, 255]);
    let named = frame.norm_of(Pixel { x: 1000, y: 500 });
    assert_eq!(named, Norm { x: 500, y: 500 });
    let back = frame.pixel_of(named);
    assert!((i64::from(back.x) - 1000).unsigned_abs() <= u64::from(bucket(screen.width)));
    assert!((i64::from(back.y) - 500).unsigned_abs() <= u64::from(bucket(screen.height)));
}

#[test]
fn a_small_capture_is_not_upscaled() {
    let screen = Size {
        width: 2,
        height: 2,
    };
    let mut rgba = Vec::new();
    rgba.extend_from_slice(&[1, 0, 0, 255]);
    rgba.extend_from_slice(&[0, 2, 0, 255]);
    rgba.extend_from_slice(&[0, 0, 3, 255]);
    rgba.extend_from_slice(&[4, 4, 4, 255]);
    let frame = Frame::downscale(screen, &rgba).expect("frame");
    assert_eq!(frame.model, screen);
    assert_eq!(frame.rgba, rgba);
}

#[test]
fn a_bad_buffer_is_refused() {
    let screen = Size {
        width: 2,
        height: 2,
    };
    let err = Frame::downscale(screen, &[0, 1, 2]).expect_err("short buffer");
    assert!(matches!(err, ScreenError::Buffer { .. }));
    let huge = Size {
        width: 9000,
        height: 10,
    };
    assert!(matches!(
        Frame::downscale(huge, &[]).expect_err("edge"),
        ScreenError::TooLarge
    ));
}

fn solid(size: Size, pixel: [u8; 4]) -> Vec<u8> {
    let Ok(n) = usize::try_from(u64::from(size.width) * u64::from(size.height)) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(n.saturating_mul(4));
    for _ in 0..n {
        out.extend_from_slice(&pixel);
    }
    out
}
