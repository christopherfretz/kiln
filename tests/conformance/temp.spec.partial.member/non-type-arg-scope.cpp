// [temp.spec.partial.member]/1, [temp.arg.nontype]: the qualifier of an
// out-of-line member definition names a partial specialization whose
// arguments include a non-dependent non-type argument (`B<1, S>`); it is
// matched after conversion, as the partial specialization's own arguments
// were (libc++ <bitset>: `__bitset<1, _Size>`).
//@ expect: run
//@ strict-diags
template <int N, int S> struct B { int f(); };
template <int S> struct B<1, S> { int g(); };
template <int S> int B<1, S>::g() { return S; }
template <class T, int S> struct C { int f(); };
template <int S> struct C<int, S> { int g(); };
template <int S> int C<int, S>::g() { return S + 1; }
template <int S, class T> struct E { int f(); };
template <class T> struct E<1, T> { int g(); };
template <class T> int E<1, T>::g() { return sizeof(T); }
template <unsigned long N, bool V> struct F { int h(); };
template <bool V> struct F<2, V> { F(); int h(); int m; };
template <bool V> F<2, V>::F() : m(V ? 7 : 8) {}
template <bool V> int F<2, V>::h() { return m; }
int main() {
  B<1, 5> b; C<int, 6> c; E<1, short> e; F<2, true> f;
  return (b.g() == 5 && c.g() == 7 && e.g() == 2 && f.h() == 7) ? 0 : 1;
}
