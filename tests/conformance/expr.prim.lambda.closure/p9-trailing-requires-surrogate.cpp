// [over.call.object]/2, [over.match.viable]/3: a lambda without a capture
// adds a surrogate call candidate through its conversion to pointer to
// function. When the call operator's trailing requires-clause is not
// satisfied, the conversion function has the same constraints
// ([expr.prim.lambda.closure]/9), so the surrogate is not viable either and
// only the call operator is shown. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:12 error "no matching function for call to object of type"
//@ diag: 20:12 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 20:12 note "conversion candidate of type 'void (*)(int)'"
//@ diag: 21:31 error "no matching function for call to object of type"
//@ diag: 22:15 note "in instantiation of function template specialization 'b<long>' requested here"
//@ diag: 21:31 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 21:31 note "conversion candidate of type 'void (*)(int)'"
//@ diag: 23:31 error "no matching function for call to object of type"
//@ diag: 24:15 note "in instantiation of function template specialization 'c<char>' requested here"
//@ diag: 23:31 note "candidate function not viable: constraints not satisfied"
//@ diag: 23:48 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
void a() { [](int) {}("x"); }
template <class U> void b() { [](int) requires(sizeof(U) > 1) {}("x"); }
template void b<long>();
template <class U> void c() { [](int) requires(sizeof(U) > 1) {}("x"); }
template void c<char>();
