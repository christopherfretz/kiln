// [expr.prim.lambda.closure]/9, [over.call.object]/2, [over.match.viable]/3:
// the trailing requires-clause of a lambda's call operator in a template
// is checked after instantiation whichever way the operator is reached:
// a call through the closure object, a call on a named closure variable,
// an explicit 'operator()' call, the conversion to pointer to function
// (which carries the same constraints) and a call with a dependent
// parameter type. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:32 error "no matching function for call to object of type"
//@ diag: 30:15 note "in instantiation of function template specialization 'f1<char>' requested here"
//@ diag: 29:32 note "candidate function not viable: constraints not satisfied"
//@ diag: 29:49 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 31:77 error "no matching function for call to object of type"
//@ diag: 32:15 note "in instantiation of function template specialization 'f2<char>' requested here"
//@ diag: 31:41 note "candidate function not viable: constraints not satisfied"
//@ diag: 31:58 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 33:79 error "invalid reference to function 'operator()': constraints not satisfied"
//@ diag: 34:15 note "in instantiation of function template specialization 'f3<char>' requested here"
//@ diag: 33:58 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 35:84 error "no viable conversion from"
//@ diag: 36:15 note "in instantiation of function template specialization 'f4<char>' requested here"
//@ diag: 35:41 note "candidate function not viable: constraints not satisfied"
//@ diag: 35:58 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 37:75 error "no matching function for call to object of type"
//@ diag: 38:15 note "in instantiation of function template specialization 'f5<char>' requested here"
//@ diag: 37:41 note "candidate function not viable: constraints not satisfied"
//@ diag: 37:56 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
template <class U> void f1() { [](int) requires(sizeof(U) > 1) {}(0); }
template void f1<char>();
template <class U> void f2() { auto l = [](int) requires(sizeof(U) > 1) {}; l(0); }
template void f2<char>();
template <class U> void f3() { auto l = [](int) requires(sizeof(U) > 1) {}; l.operator()(0); }
template void f3<char>();
template <class U> void f4() { auto l = [](int) requires(sizeof(U) > 1) {}; void (*p)(int) = l; (void)p; }
template void f4<char>();
template <class U> void f5() { auto l = [](U) requires(sizeof(U) > 1) {}; l(U{}); }
template void f5<char>();
