//! Stand-in when the `skia` feature is off: `tiny-skia` is not linked.

/// No chrome tiles.
pub(crate) struct Tiles;

#[allow(clippy::unused_self)] // the stub mirrors the real `Tiles` API
impl Tiles {
    /// Nothing to build.
    pub(crate) fn build() -> Tiles {
        Tiles
    }
    /// Zero bytes.
    pub(crate) fn bytes(&self) -> usize {
        0
    }
    /// Nothing to draw.
    pub(crate) fn blit_all(&self, _buf: &mut [u32], _width: usize) {}
}

/// Report that the build has no `tiny-skia`.
pub(crate) fn bench() {
    out!("RESULT error built_without_skia");
}
