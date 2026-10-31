// [temp.inst]/14, [except.spec]/13: a dependent noexcept-specifier of a
// class template member is instantiated only when needed, once; an
// error in it is reported with the point that needed it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:21 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 20:5 note "in instantiation of exception specification for 'f' requested here"
//@ diag: 15:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:20 note "in instantiation of exception specification for 'h' requested here"
//@ diag: 16:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 24:24 note "in instantiation of exception specification for 's' requested here"
template <class T> struct B {
  void f() noexcept(T::value);
  void g() noexcept(T::value);
  void h() noexcept(sizeof(T) > 100 ? T::value : false);
  static void s() noexcept(T::other);
};
B<int> b;
void use() {
  b.f();
  (void)noexcept(b.f());
  (void)noexcept(b.h());
}
void (*p)() = &B<int>::s;
