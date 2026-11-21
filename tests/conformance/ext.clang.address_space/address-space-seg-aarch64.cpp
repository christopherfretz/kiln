//@ expect: compile-ok
//@ strict-diags
//@ args: --target=aarch64-unknown-linux-gnu
// The x86 segment qualifiers are predefined only on x86 (WP-1813).
#if defined(__SEG_GS) || defined(__SEG_FS) || defined(__seg_gs) || defined(__seg_fs)
#error "x86 segment macros on aarch64"
#endif
int __seg_gs = 1, __seg_fs = 2;
