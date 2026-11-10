/// the recorded `E_z` history: one `nx * ny` snapshot per time step
///
/// # Memory layout
///
/// frames are stored back to back in one contiguous `Vec<f32>`. inside a
/// frame, `x` is the fastest-varying index:
///
/// ```text
/// offset = k * nx * ny  +  j * nx  +  i
/// ```
///
/// so frame `k` is a plain `&[f32]` of `ny` rows with `nx` values each, and
/// row `j = 0` is the `y = 0` wall. this is the layout a texture upload
/// expects, so the renderers can consume a frame without any reshuffling
///
/// only the `E_z` nodes are kept. the staggered `H` components and the
/// intermediate time levels needed by the leapfrog scheme are never stored
#[derive(Clone, Debug)]
pub struct Frames {
    nx: usize,
    ny: usize,
    count: usize,
    data: Vec<f32>,
}

impl Frames {
    /// creates an empty store with room for `capacity` frames of `nx * ny`
    pub fn with_capacity(nx: usize, ny: usize, capacity: usize) -> Self {
        Self {
            nx,
            ny,
            count: 0,
            data: Vec::with_capacity(nx * ny * capacity),
        }
    }

    /// appends a snapshot
    ///
    /// # Panics
    ///
    /// panics if `frame.len() != nx * ny`
    pub fn push(&mut self, frame: &[f32]) {
        assert_eq!(frame.len(), self.nx * self.ny, "frame has the wrong size");
        self.data.extend_from_slice(frame);
        self.count += 1;
    }

    /// returns frame `k` as `ny` rows of `nx` values
    ///
    /// # Panics
    ///
    /// panics if `k >= self.count()`
    pub fn frame(&self, k: usize) -> &[f32] {
        assert!(
            k < self.count,
            "frame {k} out of range (count {})",
            self.count
        );
        let len = self.nx * self.ny;
        &self.data[k * len..(k + 1) * len]
    }

    /// width of a frame (number of cells along x)
    pub fn nx(&self) -> usize {
        self.nx
    }

    /// height of a frame (number of cells along y)
    pub fn ny(&self) -> usize {
        self.ny
    }

    /// number of frames recorded so far
    pub fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_read_back() {
        let mut f = Frames::with_capacity(3, 2, 2);
        f.push(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        f.push(&[6.0, 7.0, 8.0, 9.0, 10.0, 11.0]);
        assert_eq!((f.nx(), f.ny(), f.count()), (3, 2, 2));
        assert_eq!(f.frame(0), &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        // x fastest: element (i=1, j=1) of frame 1
        assert_eq!(f.frame(1)[3 + 1], 10.0);
    }

    #[test]
    #[should_panic(expected = "wrong size")]
    fn rejects_wrong_frame_size() {
        Frames::with_capacity(3, 2, 1).push(&[0.0; 5]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn rejects_out_of_range_frame() {
        let _ = Frames::with_capacity(3, 2, 1).frame(0);
    }
}
