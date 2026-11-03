// [temp.spec.partial.member]/1, [dcl.meaning.general]/2: an out-of-line
// member of a partial specialization must match a member it declares;
// a qualifier matching no partial specialization does not name a class.
// clang 22 names the partial specialization as written and notes it;
// clang 19's wording is out-of-line-member-mismatch-clang19.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 20:36 error "out-of-line definition of 'f' does not match any declaration in 'P<T, int>'"
//@ diag: 19:27 note "P defined here"
//@ diag: 21:36 error "return type of out-of-line definition of 'P<type-parameter-0-0, int>::g' differs from that in the declaration"
//@ diag: 19:53 note "previous declaration is here"
//@ diag: 22:36 error "redefinition of 's' with a different type: 'char' vs 'int'"
//@ diag: 19:72 note "previous declaration is here"
//@ diag: 23:36 error "out-of-line definition of 'h' does not match any declaration in 'P<T, int>'"
//@ diag: 19:27 note "P defined here"
//@ diag: 24:37 error "nested name specifier 'P<T *, int>' for declaration does not refer into a class, class template or class template partial specialization"
template <class T, class U> struct P { void f(); int g(int); static int s; };
template <class T> struct P<T, int> { void f(); int g(int); static int s; };
template <class T> void P<T, int>::f(long) {}
template <class T> long P<T, int>::g(int) { return 0; }
template <class T> char P<T, int>::s = 0;
template <class T> void P<T, int>::h() {}
template <class T> void P<T*, int>::f() {}
