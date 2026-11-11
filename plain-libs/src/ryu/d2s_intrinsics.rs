use core::ptr;
pub fn div5(x: u64) -> u64 {
    x / 5
}
pub fn div10(x: u64) -> u64 {
    x / 10
}
pub fn div100(x: u64) -> u64 {
    x / 100
}
pub(crate) fn pow5_factor(mut value: u64) -> u32 {
    const M_INV_5: u64 = 14757395258967641293;
    const N_DIV_5: u64 = 3689348814741910323;
    let mut count = 0u32;
    loop {
        debug_assert!(value != 0);
        value = value.wrapping_mul(M_INV_5);
        if value > N_DIV_5 {
            break;
        }
        count += 1;
    }
    count
}
pub fn multiple_of_power_of_5(value: u64, p: u32) -> bool {
    pow5_factor(value) >= p
}
pub fn multiple_of_power_of_2(value: u64, p: u32) -> bool {
    debug_assert!(value != 0);
    debug_assert!(p < 64);
    (value & ((1u64 << p) - 1)) == 0
}
pub fn mul_shift_64(m: u64, mul: &(u64, u64), j: u32) -> u64 {
    let b0 = m as u128 * mul.0 as u128;
    let b2 = m as u128 * mul.1 as u128;
    (((b0 >> 64) + b2) >> (j - 64)) as u64
}
pub unsafe fn mul_shift_all_64(
    m: u64,
    mul: &(u64, u64),
    j: u32,
    vp: *mut u64,
    vm: *mut u64,
    mm_shift: u32,
) -> u64 {
    unsafe {
        ptr::write(vp, mul_shift_64(4 * m + 2, mul, j));
        ptr::write(vm, mul_shift_64(4 * m - 1 - mm_shift as u64, mul, j));
        mul_shift_64(4 * m, mul, j)
    }
}
