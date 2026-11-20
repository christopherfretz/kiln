// [dcl.struct.bind]/1: a structured binding declaration's type is `auto`
// with no type-constraint. [temp.param]/4, [temp.variadic]/12: a
// type-constraint `C2<T>` of a concept whose last parameter is a pack
// gives the placeholder as the first argument and `T` as the pack's
// elements, checked when the template is instantiated. Reduced from GCC's g++.dg/cpp2a/concepts-placeholder7.C.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 15:9 error "declaration cannot be declared with constrained 'auto'"
//@ diag: 21:30 error "deduced type 'int' does not satisfy 'C2'"
//@ diag: 12:37 note "because 'sizeof...(Ts) > 1' (1 > 1) evaluated to false"
template <class... Ts> concept C2 = sizeof...(Ts) > 1;
struct S { int a, b; } s;
template <class T> void h() {
  const C2<T> auto &[a, b] = s;
}
template void h<int>();
template <class T, class... Us> concept C3 = sizeof...(Us) == 2;
template <class T> int m() { C3<T, T> auto z = 1; C2<T> auto w = 2; return z + w; }
int mm = m<char>();
template <class T> int k() { C2 auto y = 1; return y; }
int kk = k<int>();
