// [basic.scope.scope]/5 (C++26, P2169R4): a `_` member of an anonymous
// union followed by a name-independent `_` in the scope its names are
// injected into is ambiguous on use. clang words it as a placeholder
// ambiguity (its `isPlaceholderVar` counts the member, which the standard
// does not make name-independent: p2169-anon-union-member.cpp), also in
// an instantiation.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:47 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 25:34 note "placeholder declared here"
//@ diag: 25:24 note "placeholder declared here"
//@ diag: 26:54 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 26:34 note "placeholder declared here"
//@ diag: 26:24 note "placeholder declared here"
//@ diag: 27:23 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 26:24 note "placeholder declared here"
//@ diag: 26:34 note "placeholder declared here"
//@ diag: 28:72 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 28:52 note "placeholder declared here"
//@ diag: 28:42 note "placeholder declared here"
//@ diag: 30:62 error "ambiguous reference to placeholder '_', which is defined multiple times"
//@ diag: 30:48 note "placeholder declared here"
//@ diag: 30:40 note "placeholder declared here"
void f() { union { int _; }; int _ = 1; (void)_; }
struct S { union { int _; }; int _; int g() { return _; } };
int h(S s) { return s._; }
template <class T> struct S2 { union { T _; }; int _; int g() { return _; } };
int x = S2<int>{}.g();
template <class T> int k() { union { T _; }; T _ = 1; return _; }
int y = k<int>();
