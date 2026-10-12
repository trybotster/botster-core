// Links libghostty-vt.a like the binding does and exercises the host allocator.
use std::ffi::c_void;
#[link(name = "ghostty-vt", kind = "static")]
extern "C" {
    fn ghostty_terminal_new(alloc: *const c_void, out: *mut *mut c_void, cols: u16, rows: u16) -> i32;
    fn ghostty_terminal_vt_write(t: *mut c_void, data: *const u8, len: usize);
    fn ghostty_terminal_free(t: *mut c_void);
}
fn main() {
    let mut total = 0usize;
    for shift in 0..27 {
        let v = vec![0u8; 1usize << shift]; // alloc_zeroed -> calloc
        total += v.len();
        let mut w: Vec<u8> = Vec::with_capacity(1 << shift); // malloc
        w.resize(1 << shift, 1);
        w.shrink_to(1); // realloc
        drop(w);
        drop(v); // free
    }
    let mut t: *mut c_void = std::ptr::null_mut();
    let rc = unsafe { ghostty_terminal_new(std::ptr::null(), &mut t, 80, 24) };
    assert_eq!(rc, 0, "terminal_new");
    let s = b"hello \x1b[31mred\x1b[0m\r\n";
    for _ in 0..2000 { unsafe { ghostty_terminal_vt_write(t, s.as_ptr(), s.len()) }; }
    unsafe { ghostty_terminal_free(t) };
    let big = vec![0u64; 1 << 24];
    println!("rust repro ok total={} big={}", total, big.len());
}
