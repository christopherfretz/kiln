// [temp.friend]/9, [temp.friend]/5, [class.friend]/6: `template <class T>
// friend int A<T>::w(int) requires C<T>;` befriends a non-template member
// of each A<T>, so it is a non-template friend declaration with a
// requires-clause, which must be a definition, and a qualified friend
// cannot be one (WP-1146). In a class template it is ill-formed; in a
// non-template class the friend's head does not make the function
// templated, so its requires-clause is rejected as for any non-templated
// function ([dcl.decl.general]/4). A member template friend
// (`template <class T> template <class U>`) may keep its trailing
// requires-clause. clang 19 and 22 agree.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:36 error "non-templated function cannot have a requires clause"
//@ diag: 27:20 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 28:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 31:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 33:58 warning "for friend class declaration is not supported; turning off access control for 'D'"
template <class T> concept C = sizeof(T) > 1;
template <class T> struct A {
  static int w(int) requires C<T>;
  template <class U> static int m(U) requires C<T>;
};
template <class T> int A<T>::w(int) requires C<T> { return 0; }
template <class T> template <class U> int A<T>::m(U) requires C<T> { return 0; }
class B {
  template <class T>
  friend int A<T>::w(int) requires C<T>;
  template <class T> template <class U> friend int A<T>::m(U) requires C<T>;
};
template <class X> struct D {
  template <class T>
  friend int A<T>::w(int) requires C<T>;
  template <class T> template <class U> friend int A<T>::m(U) requires C<T>;
};
D<int> d;
