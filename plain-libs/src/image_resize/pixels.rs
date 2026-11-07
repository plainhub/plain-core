use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::mem::size_of;
use std::slice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PixelType {
    U8x3,
    U8x4,
}

impl PixelType {
    pub fn size(&self) -> usize {
        match self {
            Self::U8x3 => 3,
            _ => 4,
        }
    }

    pub(crate) fn is_aligned(&self, buffer: &[u8]) -> bool {
        match self {
            Self::U8x3 => buffer.as_ptr().align_offset(std::mem::align_of::<U8x3>()) == 0,
            Self::U8x4 => buffer.as_ptr().align_offset(std::mem::align_of::<U8x4>()) == 0,
        }
    }
}

pub trait GetCount {
    fn count() -> usize;
}

pub struct Count<const N: usize>;

impl<const N: usize> GetCount for Count<N> {
    #[inline(always)]
    fn count() -> usize {
        N
    }
}

pub trait GetCountOfValues {
    fn count_of_values() -> usize;
}

pub struct Values<const N: usize>;

impl<const N: usize> GetCountOfValues for Values<N> {
    fn count_of_values() -> usize {
        N
    }
}

pub trait PixelComponent
where
    Self: Sized + Copy + Debug + PartialEq + 'static,
{
    type CountOfComponentValues: GetCountOfValues;

    fn count_of_values() -> usize {
        Self::CountOfComponentValues::count_of_values()
    }
}

impl PixelComponent for u8 {
    type CountOfComponentValues = Values<0x100>;
}

// Prevent users from implementing the InnerPixel trait.
mod private {
    pub trait Sealed {}
}

pub trait InnerPixel:
    private::Sealed + Copy + Clone + Sized + Debug + PartialEq + Default + Send + Sync + 'static
{
    type Component: PixelComponent;

    type CountOfComponents: GetCount;

    fn pixel_type() -> PixelType;

    fn count_of_components() -> usize {
        Self::CountOfComponents::count()
    }

    fn count_of_component_values() -> usize {
        Self::Component::count_of_values()
    }

    fn components_is_u8() -> bool {
        Self::count_of_component_values() == 256
    }

    fn size() -> usize {
        size_of::<Self>()
    }

    fn components(buf: &[Self]) -> &[Self::Component] {
        let size = buf.len() * Self::count_of_components();
        let components_ptr = buf.as_ptr() as *const Self::Component;
        unsafe { slice::from_raw_parts(components_ptr, size) }
    }

    fn components_mut(buf: &mut [Self]) -> &mut [Self::Component] {
        let size = buf.len() * Self::count_of_components();
        let components_ptr = buf.as_mut_ptr() as *mut Self::Component;
        unsafe { slice::from_raw_parts_mut(components_ptr, size) }
    }

    fn empty() -> Self {
        Self::default()
    }
}

#[derive(Copy, Clone, PartialEq, Default)]
#[repr(C)]
pub struct Pixel<T: Default, C, const COUNT_OF_COMPONENTS: usize>(
    pub T,
    PhantomData<[C; COUNT_OF_COMPONENTS]>,
)
where
    T: Sized + Copy + Clone + PartialEq + 'static,
    C: PixelComponent;

impl<T, C, const COUNT_OF_COMPONENTS: usize> Pixel<T, C, COUNT_OF_COMPONENTS>
where
    T: Sized + Copy + Clone + PartialEq + Default + 'static,
    C: PixelComponent,
{
    #[inline(always)]
    pub const fn new(v: T) -> Self {
        Self(v, PhantomData)
    }
}

macro_rules! pixel_struct {
    ($name:ident, $type:tt, $comp_type:tt, $comp_count:literal, $pixel_type:expr, $doc:expr) => {
        pub type $name = Pixel<$type, $comp_type, $comp_count>;

        impl private::Sealed for $name {}

        impl InnerPixel for $name {
            type Component = $comp_type;
            type CountOfComponents = Count<$comp_count>;

            fn pixel_type() -> PixelType {
                $pixel_type
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}{:?}", stringify!($name), self.0)
            }
        }
    };
}

pixel_struct!(
    U8x3,
    [u8; 3],
    u8,
    3,
    PixelType::U8x3,
    "Three bytes per pixel (e.g. RGB8)"
);
pixel_struct!(
    U8x4,
    [u8; 4],
    u8,
    4,
    PixelType::U8x4,
    "Four bytes per pixel (RGBA8, RGBx8, CMYK8 and other)"
);
