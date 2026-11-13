// [temp.variadic]/7, [temp.arg.explicit]/9, [temp.deduct.call]/4,
// [temp.constr.decl]: the other ways a call can fail to expand
// `TTs<Us>...` (`TTs` the enclosing class's template template parameter
// pack): more explicit arguments for `Us` than `TTs` has elements; an
// argument naming a template other than the corresponding element of
// `TTs`; and an unsatisfied requires-clause over the expansion. Each
// removes the candidate, with clang's notes.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
// (clang 19 and 22 agree.)
//@ diag: 30:3 error "no matching function for call to 'g'"
//@ diag: 25:37 note "candidate template ignored: substitution failure [with Us = <int, char, long>]: pack expansion contains parameter pack 'Us' that has a different length (at least 3 vs. 2) from outer parameter packs"
//@ diag: 31:3 error "no matching function for call to 'make'"
//@ diag: 26:47 note "candidate template ignored: substitution failure [with Us = <int, char, long>]: pack expansion contains parameter pack 'Us' that has a different length (at least 3 vs. 2) from outer parameter packs"
//@ diag: 32:3 error "no matching function for call to 'g'"
//@ diag: 25:37 note "candidate template ignored: deduced type 'Z<[...], B<char>>' of 1st parameter does not match adjusted type 'Z<[...], A<char>>' of argument [with Us = <int, char>]"
//@ diag: 33:3 error "no matching function for call to 'req'"
//@ diag: 27:80 note "candidate template ignored: constraints not satisfied [with Us = <int, char>]"
//@ diag: 27:35 note "because '__is_same(B<char>, A<char>)' evaluated to false"
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
