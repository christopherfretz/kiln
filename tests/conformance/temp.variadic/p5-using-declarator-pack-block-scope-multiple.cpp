// [namespace.udecl]/8, [temp.variadic]/5.5: at block scope a
// using-declarator pack expansion may produce at most one declaration
// (clang: "using declaration pack expansion at block scope produces
// multiple values"), reported for the instantiation that expands it to
// two; one or no element is valid.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 12:48 error "using declaration pack expansion at block scope produces multiple values"
//@ diag: 14:9 note "in instantiation of function template specialization 'g<int, char>' requested here"
template <class T> struct E { enum { A = sizeof(T) }; };
template <class... Ts> int g() { using E<Ts>::A...; return A; }
int x = g<int>();
int y = g<int, char>();
