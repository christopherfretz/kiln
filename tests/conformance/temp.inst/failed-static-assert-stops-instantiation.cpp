// [temp.inst]/3, [dcl.pre]/12: a static_assert that fails while a class
// template specialization is instantiated makes it invalid; as clang
// (`InstantiateClass`) the later members are not instantiated and no
// member function body is, so only the assertion is diagnosed (libc++
// `optional<char[20]>`, `optional<NonDestructible>`). Other invalid
// members do not stop the instantiation. The requirement prints a
// variable template-id with its arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:17 error "static assertion failed due to requirement 'sizeof(char[2]) == 0': a"
//@ diag: 36:12 note "in instantiation of template class 'A<char[2]>' requested here"
//@ diag: 33:27 note "expression evaluates to '2 == 0'"
//@ diag: 38:3 error "function cannot return array type 'char[2]'"
//@ diag: 42:12 note "in instantiation of template class 'B<char[2]>' requested here"
//@ diag: 39:17 error "static assertion failed due to requirement 'sizeof(char[2]) == 0': b"
//@ diag: 39:27 note "expression evaluates to '2 == 0'"
//@ diag: 43:31 error "function cannot return array type 'char[2]'"
//@ diag: 44:12 note "in instantiation of template class 'C<char[2]>' requested here"
//@ diag: 43:38 error "function cannot return array type 'char[2]'"
//@ diag: 47:17 error "static assertion failed due to requirement 'is_v<ND>': c"
//@ diag: 50:18 note "in instantiation of template class 'D<ND>' requested here"
//@ diag: 54:17 error "static assertion failed due to requirement 'sizeof(ND) == 0': d"
//@ diag: 57:9 note "in instantiation of template class 'E<ND>' requested here"
//@ diag: 54:27 note "expression evaluates to '1 == 0'"
//@ diag: 43:62 error "type 'char[2]' cannot be used prior to '::' because it has no members"
//@ diag: 45:11 note "in instantiation of member function 'C<char[2]>::h' requested here"
template <class T> constexpr bool is_v = false;
struct ND {
  ~ND() = delete;
};

template <class T> struct A {
  static_assert(sizeof(T) == 0, "a");
  T f();
};
A<char[2]> a;
template <class T> struct B {
  T f();
  static_assert(sizeof(T) == 0, "b");
  T g();
};
B<char[2]> b;
template <class T> struct C { T f(); T g(); int h() { return T::x; } };
C<char[2]> c;
int u = c.h();
template <class T> struct D {
  static_assert(is_v<T>, "c");
  ~D() { static_cast<T*>(nullptr)->~T(); }
};
void h() { D<ND> d; }
template <class T> struct E {
  void f() { static_cast<T*>(nullptr)->~T(); }
  int g() { return T::x; }
  static_assert(sizeof(T) == 0, "d");
};
void h2() {
  E<ND> e;
  e.f();
  e.g();
}
