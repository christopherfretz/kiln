// [dcl.fct.def.delete]/2, [temp.inst]/3: a member of a class template
// specialization is declared from the pattern's declaration, = delete
// (with its C++26 message) included, so a use of it is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:6 error "attempt to use a deleted function: no f"
//@ diag: 11:8 note "'f' has been explicitly marked deleted here"
//@ diag: 22:12 error "attempt to use a deleted function"
//@ diag: 12:15 note "'h' has been explicitly marked deleted here"
template <class T> struct A {
  void f() = delete("no f");
  static void h() = delete;
  static void k();
  A() = delete;
  ~A() = delete;
  T t;
};
static_assert(sizeof(A<int>) == sizeof(int));
void use(A<int> *p) {
  A<char>::k();
  p->f();
  A<char>::h();
}
