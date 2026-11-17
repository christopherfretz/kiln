// [temp.expl.spec]/17: a declaration shall not explicitly specialize a
// class member template if its enclosing class templates are not
// explicitly specialized as well: `template<>` under a non-empty
// template head, for a member class template, member function template
// and member variable template.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 19:47 error "cannot specialize (with 'template<>') a member of an unspecialized template"
//@ diag: 20:45 error "cannot specialize (with 'template<>') a member of an unspecialized template"
//@ diag: 21:45 error "cannot specialize (with 'template<>') a member of an unspecialized template"
//@ diag: 22:44 error "cannot specialize (with 'template<>') a member of an unspecialized template"
template <class X> struct bug {
  template <class Y> struct a {};
  template <class Y> void f(Y);
  template <class Y> static int v;
};
template <class X> template <> struct bug<X>::a<char> {};
template <class X> template <> void bug<X>::f<int>(int) {}
template <class X> template <> void bug<X>::f(long) {}
template <class X> template <> int bug<X>::v<int> = 1;
// The accepted forms: every enclosing template specialized, or the
// member template left unspecialized too.
template <> template <> struct bug<int>::a<char> {};
template <> template <class Y> struct bug<long>::a {};
template <> template <> void bug<int>::f(long) {}
template <class X> template <class Y> void bug<X>::f(Y) {}
