// [expr.prim.lambda.closure]/3, [over.match.viable]/3: the call operator of
// a lambda in a template keeps its trailing requires-clause; after
// instantiation a call whose constraints are not satisfied has no viable
// function. Inside a requires-expression the call is then invalid and the
// requirement is false. Found with WP-821. Verified against clang 19.
//@ xfail(kiln): WP-821 follow-up, the instantiated closure call operator's trailing requires-clause is not checked at the call
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:31 error "no matching function for call to object of type"
//@ diag: 16:15 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 15:31 note "candidate function not viable: constraints not satisfied"
//@ diag: 15:48 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
template <class U>
constexpr bool g() { return requires { [](int) requires(sizeof(U) > 1) {}(0); }; }
template <class U> void f() { [](int) requires(sizeof(U) > 1) {}(0); }
template void f<char>();
static_assert(g<long>() && !g<char>());
