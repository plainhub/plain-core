use std::sync::atomic::{
    AtomicBool, AtomicI8, AtomicI16, AtomicI32, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering,
};

pub trait Atom: Sized {
    type Repr: PrimitiveAtom;
    fn pack(self) -> Self::Repr;
    fn unpack(value: Self::Repr) -> Self;
}

pub trait PrimitiveAtom: Copy {
    type Storage;
    fn new(value: Self) -> Self::Storage;
    fn load(storage: &Self::Storage, order: Ordering) -> Self;
    fn store(storage: &Self::Storage, value: Self, order: Ordering);
}

macro_rules! primitive_atom {
    ($($value:ty => $storage:ty),+ $(,)?) => {$(
        impl PrimitiveAtom for $value {
            type Storage = $storage;
            fn new(value: Self) -> Self::Storage { <$storage>::new(value) }
            fn load(storage: &Self::Storage, order: Ordering) -> Self { storage.load(order) }
            fn store(storage: &Self::Storage, value: Self, order: Ordering) {
                storage.store(value, order);
            }
        }

        impl Atom for $value {
            type Repr = Self;
            fn pack(self) -> Self { self }
            fn unpack(value: Self) -> Self { value }
        }
    )+};
}

primitive_atom!(
    bool => AtomicBool,
    i8 => AtomicI8,
    i16 => AtomicI16,
    i32 => AtomicI32,
    u8 => AtomicU8,
    u16 => AtomicU16,
    u32 => AtomicU32,
    u64 => AtomicU64,
);

impl Atom for [i8; 4] {
    type Repr = u32;
    fn pack(self) -> Self::Repr {
        u32::from_ne_bytes(self.map(|byte| byte as u8))
    }
    fn unpack(value: Self::Repr) -> Self {
        value.to_ne_bytes().map(|byte| byte as i8)
    }
}

impl Atom for [i16; 4] {
    type Repr = u64;
    fn pack(self) -> Self::Repr {
        let mut bytes = [0; 8];
        for (target, value) in bytes.chunks_exact_mut(2).zip(self) {
            target.copy_from_slice(&value.to_ne_bytes());
        }
        u64::from_ne_bytes(bytes)
    }
    fn unpack(value: Self::Repr) -> Self {
        let bytes = value.to_ne_bytes();
        std::array::from_fn(|index| i16::from_ne_bytes([bytes[index * 2], bytes[index * 2 + 1]]))
    }
}

#[repr(transparent)]
pub struct Atomic<T: Atom>(<T::Repr as PrimitiveAtom>::Storage);

impl<T: Atom> Atomic<T> {
    pub fn new(value: T) -> Self {
        Self(T::Repr::new(value.pack()))
    }

    pub fn load(&self, order: Ordering) -> T {
        T::unpack(T::Repr::load(&self.0, order))
    }

    pub fn store(&self, value: T, order: Ordering) {
        T::Repr::store(&self.0, value.pack(), order);
    }
}

impl<T: Atom<Repr = u32>> Atomic<T> {
    pub fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T> {
        self.0
            .compare_exchange(current.pack(), new.pack(), success, failure)
            .map(T::unpack)
            .map_err(T::unpack)
    }
}

impl<T: Atom + Default> Default for Atomic<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// A [`Atomic`] type that can only be accessed
/// through [`Ordering::Relaxed`] loads and stores,
/// which are essentially normal loads and stores.
///
/// This prevents the usage of methods like [`AtomicU8::fetch_or`]
/// that compile down to contended `lock *` instructions
/// even with [`Ordering::Relaxed`].
///
/// [`AtomicU8::fetch_or`]: std::sync::atomic::AtomicU8::fetch_or
#[derive(Default)]
pub struct RelaxedAtomic<T: Atom> {
    inner: Atomic<T>,
}

impl<T: Atom> RelaxedAtomic<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Atomic::new(value),
        }
    }

    pub fn get(&self) -> T {
        self.inner.load(Ordering::Relaxed)
    }

    pub fn set(&self, value: T) {
        self.inner.store(value, Ordering::Relaxed);
    }

    pub fn update(&self, f: impl Fn(T) -> T) {
        self.set(f(self.get()));
    }
}

impl<T: Atom + Copy> RelaxedAtomic<T> {
    pub fn get_update(&self, f: impl Fn(T) -> T) -> T {
        let old = self.get();
        self.set(f(old));
        old
    }
}

impl<T: Atom> Clone for RelaxedAtomic<T> {
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl<T: Atom> From<T> for RelaxedAtomic<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
