// [temp.inst]/2: a class whose implicit instantiation failed (here a
// static_assert) is invalid; clang reports the failure once and then
// initializes no object of it quietly, list, functional-cast and
// static_cast forms alike (libc++ out_ptr / inout_ptr).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:17 error "static assertion failed due to requirement 'sizeof(int) == 0': not allowed"
//@ diag: 15:9 note "in instantiation of template class 'Checked<int>' requested here"
//@ diag: 11:27 note "expression evaluates to '4 == 0'"
template <class T> struct Checked {
  static_assert(sizeof(T) == 0, "not allowed");
  Checked(int, int);
};
void f(int *p) {
  (void)static_cast<Checked<int>>(p);
  Checked<int> c{1, 2};
  (void)Checked<int>(1, 2);
}
