// [over.match.class.deduct]/1.1, [dcl.fct.spec]/3, [over.match.list]/1:
// an implicit guide formed from a constructor with a value-dependent
// `explicit(b)` is explicit only where `b` is true for the deduced
// arguments; `C c = {1, 0};` (V = int, sizeof 4) uses a non-explicit
// guide in copy-list-initialization.
//@ expect: run
//@ xfail(kiln): WP-1737 follow-up, an implicit deduction guide keeps its constructor's unresolved explicit(b) as explicit
//@ stdout: C 4 B
#include <cstdio>
template <typename V> struct C {
  explicit(sizeof(V) > 4) C(V, int) { std::printf("C %d ", int(sizeof(V))); }
};
template <typename V, int N> struct B {
  explicit(N != 5) B(V, int) { std::printf("B\n"); }
};
template <class T> using U = B<T, 5>;
int main() {
  C c = {1, 0};
  U u = {1, 0};
}
