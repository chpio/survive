//! The raster of square cells over the whole world that the flow field and the sight work on. Its
//! origin is the world's origin, and maps are placed in whole tiles, so their edges lie on it.

use bevy::prelude::*;
use std::ops::{Index, IndexMut};

/// Edge length of a cell in px (a quarter tile). Small enough that a corridor one tile wide still
/// has free cells for small enemies after the obstacles around it are inflated.
pub const CELL_SIZE: f32 = 4.;

/// Cell containing `position`.
pub fn cell(position: Vec2) -> IVec2 {
    (position / CELL_SIZE).floor().as_ivec2()
}

/// World position of the center of `cell`.
pub fn center(cell: IVec2) -> Vec2 {
    (cell.as_vec2() + 0.5) * CELL_SIZE
}

/// Rectangle of cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellRect {
    /// Cell at the bottom-left corner
    pub min: IVec2,
    /// Number of columns and rows
    pub size: IVec2,
}

impl CellRect {
    /// Empty if `size` is negative in either direction.
    pub fn new(min: IVec2, size: IVec2) -> Self {
        Self {
            min,
            size: size.max(IVec2::ZERO),
        }
    }

    /// Square reaching `radius` cells from `center` in each direction.
    pub fn around(center: IVec2, radius: i32) -> Self {
        Self::new(center - radius, IVec2::splat(2 * radius + 1))
    }

    /// Cells that `area` covers at least partly.
    pub fn covering(area: Rect) -> Self {
        let min = cell(area.min);
        Self::new(min, (area.max / CELL_SIZE).ceil().as_ivec2() - min)
    }

    /// Cells whose center lies in `area`.
    pub fn centers_in(area: Rect) -> Self {
        let min = (area.min / CELL_SIZE - 0.5).ceil().as_ivec2();
        let max = (area.max / CELL_SIZE - 0.5).floor().as_ivec2() + 1;
        Self::new(min, max - min)
    }

    /// The cell after the top-right corner, outside the rectangle.
    pub fn max(self) -> IVec2 {
        self.min + self.size
    }

    /// The rectangle grown by `cells` on each side.
    pub fn grow(self, cells: i32) -> Self {
        Self::new(self.min - cells, self.size + 2 * cells)
    }

    /// Cells in both rectangles.
    pub fn intersect(self, other: Self) -> Self {
        let min = self.min.max(other.min);
        Self::new(min, self.max().min(other.max()) - min)
    }

    pub fn len(self) -> usize {
        (self.size.x * self.size.y) as usize
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Index of `cell` when counting the cells row by row from the bottom, `None` outside.
    pub fn index(self, cell: IVec2) -> Option<usize> {
        let local = cell - self.min;
        (local.cmpge(IVec2::ZERO).all() && local.cmplt(self.size).all())
            .then(|| (local.y * self.size.x + local.x) as usize)
    }

    /// Inverse of [`Self::index`].
    pub fn cell(self, index: usize) -> IVec2 {
        self.min + IVec2::new(index as i32 % self.size.x, index as i32 / self.size.x)
    }

    /// All cells row by row from the bottom.
    pub fn cells(self) -> impl Iterator<Item = IVec2> {
        let max = self.max();
        (self.min.y..max.y).flat_map(move |y| (self.min.x..max.x).map(move |x| IVec2::new(x, y)))
    }

    /// Area of the world the cells cover.
    pub fn world_rect(self) -> Rect {
        Rect::from_corners(
            self.min.as_vec2() * CELL_SIZE,
            self.max().as_vec2() * CELL_SIZE,
        )
    }
}

/// A value per cell of a [`CellRect`], indexed by cell or by the cell's index in it.
#[derive(Clone, Debug)]
pub struct Raster<T> {
    rect: CellRect,
    /// Row by row from the bottom
    values: Vec<T>,
}

impl<T: Clone> Raster<T> {
    pub fn new(rect: CellRect, value: T) -> Self {
        Self {
            rect,
            values: vec![value; rect.len()],
        }
    }

    pub fn rect(&self) -> CellRect {
        self.rect
    }

    /// Moves the raster to start at the cell `min`, keeping its size, and sets all values to
    /// `value`.
    pub fn reset(&mut self, min: IVec2, value: T) {
        self.rect.min = min;
        self.values.fill(value);
    }

    /// Value of `cell`, `None` outside the raster.
    pub fn get(&self, cell: IVec2) -> Option<&T> {
        self.rect.index(cell).map(|i| &self.values[i])
    }

    /// All values, row by row from the bottom.
    pub fn values(&self) -> &[T] {
        &self.values
    }
}

impl<T> Index<usize> for Raster<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        &self.values[index]
    }
}

impl<T> IndexMut<usize> for Raster<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.values[index]
    }
}

/// Panics outside the raster.
impl<T> Index<IVec2> for Raster<T> {
    type Output = T;

    fn index(&self, cell: IVec2) -> &T {
        let index = self.rect.index(cell).expect("cell outside the raster");
        &self.values[index]
    }
}

/// Panics outside the raster.
impl<T> IndexMut<IVec2> for Raster<T> {
    fn index_mut(&mut self, cell: IVec2) -> &mut T {
        let index = self.rect.index(cell).expect("cell outside the raster");
        &mut self.values[index]
    }
}
