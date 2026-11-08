// [temp.point]/7, [temp.inst]/11: the member functions used here are
// instantiated at the end of the translation unit, after `after`'s error.
// With -ferror-limit=2, A<int>::f's error is the last one shown and B<int>::f's
// becomes the fatal "too many errors emitted, stopping now"; no instantiation
// runs after it (clang's InstantiatingTemplate refuses every instantiation
// once a fatal error was emitted; spec/sema.md "Error recovery", WP-826).
// fatal-error-stops-pending-instantiation.cpp checks that the work is not done.
//@ expect: compile-fail
//@ args: -ferror-limit=2
//@ strict-diags
//@ diag: 20:20 error "cannot initialize a variable of type 'int' with an lvalue of type 'const char[3]'"
//@ diag: 15:49 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 19:22 note "in instantiation of member function 'A<int>::f' requested here"
// (then the locationless fatal "too many errors emitted", which diag: cannot name)
template <class T> struct A { static void f() { T::a(); } };
template <class T> struct B { static void f() { T::b(); } };
template <class T> struct C { static void f() { T::c(); } };
template <class T> void g() { T::g(); }
void use() { A<int>::f(); B<int>::f(); C<int>::f(); g<int>(); }
void after() { int x = "no"; }
