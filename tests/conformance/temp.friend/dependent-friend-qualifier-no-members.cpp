// [temp.res.general]/6, [basic.lookup.qual]/1: `friend void T::f();` in a
// class template is checked when the class is instantiated; a template
// argument that is not a class makes the nested-name-specifier `T::` name
// a type with no members. clang reports that at the specifier, where kiln
// reported it at the friend declaration's start (found running GCC's C++
// testsuite). The rejected sibling: a class argument without the member.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 20:8 note "in instantiation of template class 'Y<int>' requested here"
//@ diag: 16:14 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 17:31 error "type 'int' cannot be used prior to '::' because it has no members"

template <class T> struct Y {
  friend void T::f();
  friend int T::g(int);
  template <class U> friend U T::h(U);
};
struct S { void f(); int g(int); template <class U> U h(U); };
Y<int> y;
Y<S> ok;
