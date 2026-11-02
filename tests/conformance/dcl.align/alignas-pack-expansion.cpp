// [dcl.align]/1, [temp.variadic]/5: alignas(pack...) yields one alignment per
// element and the strictest wins; an unexpanded pack, an expansion with no
// pack, and a non-power-of-2 alignment (at the attribute) are errors
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:43 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 29:43 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 30:53 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 31:33 error "requested alignment is not a power of 2"
//@ diag: 32:6 note "in instantiation of template class 'V<3>' requested here"
//@ diag: 33:33 error "requested alignment is not a power of 2"
//@ diag: 34:6 note "in instantiation of template class 'X<2>' requested here"
//@ diag: 35:1 error "requested alignment is not a power of 2"
template <class... Ts> struct alignas(Ts...) S { char c; };
static_assert(alignof(S<char, double, int>) == alignof(double));
template <class... Ts> struct T2 { alignas(Ts...) char buf[4]; };
static_assert(alignof(T2<short, long>) == alignof(long));
template <int... Ns> struct U { alignas(Ns...) char c; };
static_assert(alignof(U<2, 16, 4>) == 16);
template <int... Ns> struct U1 { alignas(Ns * 2 ...) char c; };
static_assert(alignof(U1<1, 4>) == 8);
template <class... Ts> struct E { alignas(Ts...) int c; };
static_assert(alignof(E<>) == 4);
template <class... Ts> struct O { struct I { alignas(Ts...) char c; }; };
static_assert(alignof(O<short, int>::I) == 4);
template <class... Ts> void f() { alignas(Ts...) int x; (void)x; }
template void f<double>();
template <class... Ts> struct W { alignas(Ts) int c; };
template <class... Ts> struct Y { alignas(sizeof(Ts)) int c; };
template <class T, class... Ts> struct Z { alignas(T...) int c; };
template <int... Ns> struct V { alignas(Ns...) char c; };
V<3> v;
template <int... Ns> struct X { alignas(Ns + 1 ...) char c; };
X<2> x;
alignas(3) char g;
