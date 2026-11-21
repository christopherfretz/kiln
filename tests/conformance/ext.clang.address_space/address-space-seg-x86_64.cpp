//@ expect: compile-ok
//@ strict-diags
//@ args: --target=x86_64-unknown-linux-gnu
// On x86 `__seg_gs`, `__seg_fs`, `__SEG_GS` and `__SEG_FS` are predefined
// (WP-1813), the qualifiers as address_space(256) and address_space(257).
#if !defined(__SEG_GS) || !defined(__SEG_FS) || !defined(__seg_gs) || !defined(__seg_fs)
#error "x86 segment macros missing"
#endif
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;
static_assert(same_v<__seg_gs int, int __attribute__((address_space(256)))>);
static_assert(same_v<__seg_fs int, int __attribute__((address_space(257)))>);
