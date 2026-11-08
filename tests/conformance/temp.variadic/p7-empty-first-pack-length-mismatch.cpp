// [temp.variadic]/7: the packs of one expansion have the same length even
// when the first one reached is empty; the mismatch is the error clang
// reports (a function body), and the deduction failure in a return type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:73 error "pack expansion contains parameter packs 'T' and 'U' that have different lengths (0 vs. 1)"
//@ diag: 16:14 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 17:15 error "no matching function for call to 'h'"
//@ diag: 15:44 note "candidate template ignored: substitution failure [with I = <>, J = <1>]: pack expansion contains parameter packs 'I' and 'J' that have different lengths (0 vs. 1)"
template<class... T> struct P {};
template<class A, class B> struct pr {};
void g(...);
template<class... T, class... U> void f(P<T...>, P<U...>) { g(pr<T, U>{}...); }
template<int... I> struct S {};
template<int... I, int... J> S<(I * J)...> h(S<I...>, S<J...>);
void use() { f(P<>{}, P<int>{}); }
void use2() { h(S<>{}, S<1>{}); }
