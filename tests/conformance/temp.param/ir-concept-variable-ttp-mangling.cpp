// [temp.param]/1 (P2841R7 concept and variable template template
// parameters), [temp.names], [temp.deduct.type]: specializations of templates
// parameterised by a concept or a variable template are emitted and mangled
// as clang 22 does. A concept or variable-template argument is its template
// name (`_Z1fI3IntiEiT0_`); a use `C<T>` / `V<T>` in a signature is an
// unresolved name (`X1CIT0_EE`); a type-constraint naming a concept
// template parameter mangles as the nested name `N <owner template>
// <parameter name> E` (`TkN2ct1CEi`, spec/abi.md "Concept and variable
// template template parameters"). Compared with clang 22's IR exactly.
// WP-746.
//@ expect: ir
//@ requires: clang>=22
//@ args: -std=c++26 -ftrivial-auto-var-init=uninitialized
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr int sz = sizeof(T) == sizeof(char) ? 1 : 7;
template <int N> struct I {};

template <template <class> concept C, class T> int f(T) { return C<T> ? 1 : 2; }
template <template <class> auto V, class T> int g(T) { return V<T>; }
template <template <class> concept C> struct S { static int h() { return C<int>; } };
template <template <class> auto V> struct W { static int h() { return V<char>; } };
template <template <class> concept C, class T> requires C<T> int k(T) { return 3; }
template <template <class> concept... Cs> int pk() { return sizeof...(Cs); }
template <template <class> concept C, C T> int ct(T) { return 4; }
template <template <class...> concept C> int fv() { return C<int>; }
template <template <class...> auto V> int gv() { return V<int>; }
template <template <class> concept C, class T> struct X {
  static int h(C auto) { return 1; }
  template <C U> static int m(U) { return 2; }
};
template <template <class> concept C> int pl(C auto) { return 5; }

namespace ns {
template <template <class> concept C, C T> int ct(T, T) { return 4; }
template <template <class> concept C> struct X {
  template <C U> static int m(U, ns::X<C> *) { return 2; }
};
} // namespace ns

template <template <class> auto V, class T> I<V<T>> r(T) { return {}; }
template <template <class> concept C, class T> I<C<T>> s(T) { return {}; }
template <template <class> concept... Cs, class T> I<(Cs<T> && ...)> u(T) { return {}; }
template <template <class> concept C, class T> requires(!C<T *>) int w(T) { return 1; }
template <template <class> auto V> struct Q {};
template <template <class> auto V> void tq(Q<V>) {}
template <template <class> concept C> struct QC {};
template <template <class> concept C> void tc(QC<C>) {}

int main() {
  int t = f<Int>(1) + g<sz>(1) + S<Int>::h() + W<sz>::h() + k<Int>(1) +
          pk<Int, Int>() + ct<Int>(1);
  t += fv<Int>() + gv<sz>() + X<Int, char>::h(1) + X<Int, char>::m(1) + pl<Int>(1);
  t += ns::ct<Int>(1, 1) + ns::X<Int>::m(1, nullptr);
  r<sz>(1);
  s<Int>(1);
  u<Int, Int>(1);
  t += w<Int>(1);
  tq(Q<sz>{});
  tc(QC<Int>{});
  return t;
}
