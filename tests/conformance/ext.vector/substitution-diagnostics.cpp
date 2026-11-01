// A vector attribute whose size or element type is dependent is checked
// when substituted, and diagnosed at the attribute; as clang does, the
// type is then left unchanged, so the member typedef still exists
// (WP-156).
//@ expect: compile-fail
//@ diag: 15:24 error "zero vector size"
//@ diag: 18:1 note "in instantiation of template type alias 'V' requested here"
//@ diag: 15:24 error "invalid vector element type 'int *'"
//@ diag: 19:1 note "in instantiation of template type alias 'V' requested here"
//@ diag: 17:42 error "vector size not an integral multiple of component size"
//@ diag: 20:1 note "in instantiation of template class 'S<int, 3>' requested here"
//@ diag: 17:42 error "zero vector size"
//@ diag: 21:1 note "in instantiation of template class 'S<int, 0>' requested here"
template <class T, unsigned N>
using V __attribute__((__ext_vector_type__(N))) = T;
template <class T, int N>
struct S { typedef T type __attribute__((vector_size(N))); };
V<int, 0> a;
V<int*, 4> b;
S<int, 3>::type c;
S<int, 0>::type d;
