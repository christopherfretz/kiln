// [temp.inst]/11 and [temp.point]/7: the member functions used here are
// instantiated at the end of the translation unit. clang 19's
// InstantiatingTemplate refuses every instantiation once a fatal error has
// been emitted (Diags.hasFatalErrorOccurred()), and kiln does the same
// (spec/sema.md "Error recovery", WP-826): with -ferror-limit=1, B<int>::f's
// error becomes "too many errors emitted", so C<int>::f and g<int> are never
// instantiated. The instantiation report shows A::f and B::f only, and no
// function row for g.
//@ expect: compile-fail
//@ args: -ferror-limit=1
//@ strict-diags
//@ diag: 17:42 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:23 note "in instantiation of member function 'A<int>::f' requested here"
// (then the locationless fatal "too many errors emitted", which diag: cannot name)
//@ inst-report: fatal-error-stops-pending-instantiation.json

template <class T> struct A { void f() { T::a(); } };
template <class T> struct B { void f() { T::b(); } };
template <class T> struct C { void f() { T::c(); } };
template <class T> void g() { T::g(); }

void use() { A<int>().f(); B<int>().f(); C<int>().f(); g<int>(); }
