use std::ops::{Index, IndexMut};

#[derive(Clone, Debug)]
pub struct Array3D<T> {
    width: usize,
    height: usize,
    depth: usize,
    data: Vec<T>,
}

impl<T: Default + Clone> Array3D<T> {
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self {
            width,
            height,
            depth,
            data: vec![T::default(); width * height * depth],
        }
    }

    // reset all elements to T::default()
    pub fn set_null(&mut self) {
        self.data.fill(T::default());
    }
}

impl<T> Array3D<T> {
    #[inline]
    fn index_of(&self, x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < self.width && y < self.height && z < self.depth);
        x + y * self.width + z * self.width * self.height
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&T> {
        if x < self.width && y < self.height && z < self.depth {
            self.data
                .get(x + y * self.width + z * self.width * self.height)
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, val: T) {
        let i = self.index_of(x, y, z);
        self.data[i] = val;
    }

    pub fn dims(&self) -> (usize, usize, usize) {
        (self.width, self.height, self.depth)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

impl<T> Index<(usize, usize, usize)> for Array3D<T> {
    type Output = T;
    #[inline]
    fn index(&self, (x, y, z): (usize, usize, usize)) -> &T {
        &self.data[self.index_of(x, y, z)]
    }
}

impl<T> IndexMut<(usize, usize, usize)> for Array3D<T> {
    #[inline]
    fn index_mut(&mut self, (x, y, z): (usize, usize, usize)) -> &mut T {
        let i = self.index_of(x, y, z);
        &mut self.data[i]
    }
}
