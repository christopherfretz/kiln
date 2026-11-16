// [namespace.udecl]/3: a member using-declarator whose nested-name-specifier
// names the class template's own current instantiation finds nothing when
// no member of that name is declared in the class itself; clang reports "no
// member named" at the definition, with dependent bases too (kiln reported
// "using declaration refers to its own class" at instantiation; found
// running GCC's C++ testsuite). The C++98 access-declaration form is in the
// -cxx98 sibling. A base class named in the specifier is accepted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "no member named 'yyy' in 'D<T>'"
//@ diag: 16:12 error "no member named 'xxx' in 'D<T>'"

template <class T> struct I { void g(); int h; };
template <class T> struct D : I<T> {
  using D<T>::yyy;
  using D::xxx;
  using I<T>::g;
  using I<T>::h;
};
D<int> d;
