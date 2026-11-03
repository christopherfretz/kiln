// [dcl.ambig.res]/1, [temp.explicit]/3: after a template-id declarator-id a
// `(` always starts a parameter clause (clang takes no direct-initializer
// after a non-identifier name), so `template int t<char>(1);` declares a
// function: "expected parameter declarator" and the missing `)` at the `1`,
// then no function template matches. With `=` or braces the initializer is
// a definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:22 error "expected parameter declarator"
//@ diag: 38:22 error "expected ')'"
//@ diag: 38:21 note "to match this '('"
//@ diag: 38:14 error "explicit instantiation of 't' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 39:25 error "expected parameter declarator"
//@ diag: 39:25 error "expected ')'"
//@ diag: 39:24 note "to match this '('"
//@ diag: 39:17 error "explicit instantiation of 's' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 42:23 error "expected parameter declarator"
//@ diag: 42:23 error "expected ')'"
//@ diag: 42:22 note "to match this '('"
//@ diag: 42:15 error "explicit instantiation of 'fn' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 41:24 note "candidate template ignored: could not match 'void (int)' against 'void ()'"
//@ diag: 43:14 error "explicit template instantiation cannot have a definition; if this definition is meant to be an explicit specialization, add '<>' after the 'template' keyword"
//@ diag: 44:14 error "explicit template instantiation cannot have a definition; if this definition is meant to be an explicit specialization, add '<>' after the 'template' keyword"
//@ diag: 45:25 error "expected parameter declarator"
//@ diag: 45:25 error "expected ')'"
//@ diag: 45:24 note "to match this '('"
//@ diag: 45:16 error "no function template matches function template specialization 't'"
//@ diag: 46:16 error "no function template matches function template specialization 't'"
//@ diag: 47:26 error "expected parameter declarator"
//@ diag: 47:26 error "expected ')'"
//@ diag: 47:25 note "to match this '('"
//@ diag: 47:17 error "no function template matches function template specialization 'fn'"
//@ diag: 41:24 note "candidate template ignored: could not match 'void (char)' against 'void ()'"
//@ diag: 48:27 error "expected parameter declarator"
template<class T> int t = 0;
struct S { template<class T> static int s; };
template<class T> int S::s = 1;
template int t<char>(1);
template int S::s<char>(1);
template int t<long>;
template<class T> void fn(T);
template void fn<int>(1);
template int t<short> = 2;
template int t<int>{3};
template<> int t<float>(1);
template<> int t<double>(int);
template<> void fn<char>(1);
template<> int S::s<long>(2);
int after;
