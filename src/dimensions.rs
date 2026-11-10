use std::ops::{Index, IndexMut};

/// a dense, row-major-ish 3D array stored in one contiguous `Vec<T>`
///
/// # Memory layout
///
/// `x` is the fastest-varying index, then `y`, then `z`:
///
/// ```
/// linear_index = x + y * width + z * width * height
/// ```
/// `z` is used as the (doubled) time index,
/// so each time slice is one contiguous `width * height` block
///
/// # Indexing
///
/// elements are accessed with a tuple, i.e. `a[(x, y, z)]`, or with
/// [`Array3D::set`]. [`Array3D::get`] is used for a bounds-checked
/// access that returns an `Option`
#[derive(Clone, Debug)]
pub struct Array3D<T> {
    width: usize,
    height: usize,
    depth: usize,
    data: Vec<T>,
}

impl<T: Default + Clone> Array3D<T> {
    /// creates a `width * height * depth` array with every element set to
    /// `T::default()`
    ///
    /// allocates `width * height * depth * size_of::<T>()` bytes up front
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self {
            width,
            height,
            depth,
            data: vec![T::default(); width * height * depth],
        }
    }

    /// resets all elements to T::default(),
    /// keeping the dimensions and the allocation.
    #[allow(dead_code)]
    pub fn set_null(&mut self) {
        self.data.fill(T::default());
    }
}

impl<T> Array3D<T> {
    /// converts `(x, y, z)` to an offset into the flat backing vector
    #[inline]
    fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < self.width && y < self.height && z < self.depth);
        x + y * self.width + z * self.width * self.height
    }

    /// bounds-checked element access
    ///
    /// returns `None` if any of `x`, `y`, `z` is outside the array
    /// dimensions, otherwise a reference to the element
    #[allow(dead_code)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&T> {
        if x < self.width && y < self.height && z < self.depth {
            self.data
                .get(x + y * self.width + z * self.width * self.height)
        } else {
            None
        }
    }

    /// overwrites the element at `(x, y, z)` with `val`
    pub fn set(&mut self, x: usize, y: usize, z: usize, val: T) {
        let i = self.index_of(x, y, z);
        self.data[i] = val;
    }

    /// returns the dimensions as `(width, height, depth)`
    pub fn dims(&self) -> (usize, usize, usize) {
        (self.width, self.height, self.depth)
    }

    /// returns the raw backing storage
    /// in the layout described on [`Array3D`]
    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

impl<T> Index<(usize, usize, usize)> for Array3D<T> {
    type Output = T;

    /// immutable access via `array[(x, y, z)]`
    #[inline]
    fn index(&self, (x, y, z): (usize, usize, usize)) -> &T {
        &self.data[self.index_of(x, y, z)]
    }
}

impl<T> IndexMut<(usize, usize, usize)> for Array3D<T> {
    /// mutable access via `array[(x, y, z)]`
    #[inline]
    fn index_mut(&mut self, (x, y, z): (usize, usize, usize)) -> &mut T {
        let i = self.index_of(x, y, z);
        &mut self.data[i]
    }
}
