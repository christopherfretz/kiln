// [temp.variadic]/7, [temp.deduct.general]/8: in `Z<TTs<Us>...>`, a
// member template's parameter type with `TTs` the enclosing class's
// template template parameter pack, both packs must have the same length:
// deducing `Us` from an argument with fewer elements is a substitution
// failure naming the outer pack, as is deducing it from one with more (a
// diagnostic, never a crash).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:3 error "no matching function for call to 'g'"
//@ diag: 15:87 note "candidate template ignored: substitution failure [with Us = <int>]: pack expansion contains parameter pack 'Us' that has a different length (2 vs. 1) from outer parameter packs"
//@ diag: 18:3 error "no matching function for call to 'g'"
//@ diag: 15:87 note "candidate template ignored: substitution failure [with Us = <int, char, long>]: pack expansion contains parameter pack 'Us' that has a different length (2 vs. 3) from outer parameter packs"
template <class... Ts> struct Z {};
template <class> struct A {};
template <template <class> class... TTs> struct S { template <class... Us> static int g(Z<TTs<Us>...>); };
void use() {
  S<A, A>::g(Z<A<int>>{});
  S<A, A>::g(Z<A<int>, A<char>, A<long>>{});
}
