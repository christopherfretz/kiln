// [temp.deduct.general]/2-5, [temp.arg.explicit]/9, [temp.variadic]/7:
// the explicit-argument substitution into a trailing pack's pattern
// leaves valid patterns alone. A valid `T` keeps the candidate with `Ts`
// empty or deduced; a deduced `T` whose pattern would be invalid is fine
// when `Ts` deduces empty, since the final substitution expands the
// pattern zero times and never forms it (here and through a deduction
// guide); and an explicitly given pack shorter than another pack of the
// pattern is a lower bound that deduction extends, not a length conflict
// (GCC 16 rejects `S<int, long>::f<char>`; clang accepts it, as /9 says).
// The check keeps the given pack's expansion retained, so a use of the
// whole pack (`X<Ts...>`, defined only for two arguments) is not formed
// with the given `Ts = {char}` alone, in the trailing pattern (GCC 16
// rejects `e<char>`) or in another parameter.
//@ expect: compile-ok
//@ strict-diags
template <class...> struct L {};
template <class, class> struct P {};
struct A { using type = int; };
template <class T, class... Ts> void h(L<Ts, typename T::type>...) {}
template <class T, class... Ts> void d(T, L<Ts, typename T::type>...) {}
template <class T> struct C { template <class... Ts> C(T, Ts...) {} };
template <class T, class... Ts> C(T, L<Ts, typename T::type>...) -> C<T>;
template <class... Ts> struct X;
template <class A, class B> struct X<A, B> { using type = int; };
template <class... Ts> void e(L<typename X<Ts...>::type, Ts>...) {}
template <class... Ts> void x(typename X<Ts...>::type, L<Ts...>) {}
template <class... Vs> struct S {
  template <class... Ts> static int f(P<Ts, Vs>...) { return 0; }
};
void g() {
  h<A>();
  h<A>(L<char, int>{}, L<long, int>{});
  d(0);
  C c(0);
  (void)c;
  S<int, long>::f<char>(P<char, int>{}, P<short, long>{});
  e<char>(L<int, char>{}, L<int, short>{});
  x<char>(0, L<char, short>{});
}
