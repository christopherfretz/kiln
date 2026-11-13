// [temp.variadic]/7, [temp.arg.explicit]/9, [temp.deduct.call]/4,
// [temp.constr.decl]: the other ways a call can fail to expand
// `TTs<Us>...` (`TTs` the enclosing class's template template parameter
// pack): more explicit arguments for `Us` than `TTs` has elements; an
// argument naming a template other than the corresponding element of
// `TTs`; and an unsatisfied requires-clause over the expansion. Each
// removes the candidate (the notes' wording differs from clang's; only the
// errors are pinned).
//@ expect: compile-fail
//@ diag: 23:3 error "no matching function for call to 'g'"
//@ diag: 24:3 error "no matching function for call to 'make'"
//@ diag: 25:3 error "no matching function for call to 'g'"
//@ diag: 26:3 error "no matching function for call to 'req'"
template <class... Ts> struct Z {};
template <class> struct A {};
template <class> struct B {};
template <template <class> class... TTs> struct S {
  template <class... Us> static int g(Z<TTs<Us>...>);
  template <class... Us> static Z<TTs<Us>...> make();
  template <class... Us> requires(__is_same(TTs<Us>, A<Us>) && ...) static int req(Us...);
};
void use() {
  S<A, B>::g<int, char, long>(Z<A<int>, B<char>>{});
  S<A, B>::make<int, char, long>();
  S<A, B>::g(Z<A<int>, A<char>>{});
  S<A, B>::req(1, 'c');
}
