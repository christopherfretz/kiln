// [temp.over.link]/5, [temp.dep.type]/1: the in-class `I<vt<3>>` and
// the out-of-line `I<C<X>::template vt<3>>` name the same variable
// template specialization of the current instantiation, so the
// definition matches its declaration. clang 22 accepts it; clang 19
// (and kiln, which profiles the qualifier) rejects it as a redefinition
// mismatch (WP-1169).
//@ expect: run
//@ requires: clang>=22
//@ xfail(kiln): WP-1169 follow-up, a variable template-id's profile keeps its written qualifier (`C<X>::template vt<3>` is not `vt<3>`)
//@ stdout: 6
#include <cstdio>
template <int N> struct I { static constexpr int v = N; };
template <class X> struct C {
  template <int K> static constexpr int vt = K * 2;
  I<vt<3>> c();
};
template <class X> I<C<X>::template vt<3>> C<X>::c() { return {}; }
int main() {
  C<int> x;
  std::printf("%d\n", decltype(x.c())::v);
}
