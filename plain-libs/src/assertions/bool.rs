#[derive(Clone, Copy)]
pub struct True;
#[derive(Clone, Copy)]
pub struct False;

impl True {
    pub const fn not(&self) -> &'static False {
        &False
    }
    pub const fn and<'a, T>(&self, other: &'a T) -> &'a T {
        other
    }
    pub const fn or<T>(&self, _: &T) -> &'static True {
        &True
    }
    pub const fn value(&self) -> bool {
        true
    }
}

impl False {
    pub const fn not(&self) -> &'static True {
        &True
    }
    pub const fn and<T>(&self, _: &T) -> &'static False {
        &False
    }
    pub const fn or<'a, T>(&self, other: &'a T) -> &'a T {
        other
    }
    pub const fn value(&self) -> bool {
        false
    }
}
