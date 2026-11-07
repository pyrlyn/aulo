Outcome: `crates/aulo-screen` turns RGBA capture bytes into a model frame (long edge 1280) and 0–999 coordinates. A 1× point and the same point on a 2× capture normalize equally, and the pixel round-trip stays inside one bucket. xcap is not linked; its Linux backend pulls PipeWire and an unpinned git dependency.

The outcome as the old `plan.md` card recorded it:

Outcome: `aulo-screen` maps a capture pixel to 0–999 and back. The same relative point on a 1440×900 capture and a 2× Retina capture is (500, 500), and a grid of points round-trips inside one bucket (`extent / 1000` pixels). Frames downscale so the long edge is 1280, keeping aspect; a smaller capture is left as it is. Empty, short, and oversized buffers are refused. The bytes are the RGBA shape xcap returns. xcap itself is not linked: 0.9.8's Linux backend depends on PipeWire and an unpinned git crate, which would make every CI target build that stack for a geometry test.
