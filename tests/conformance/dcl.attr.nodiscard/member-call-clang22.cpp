// [dcl.attr.nodiscard]/4, [dcl.attr.deprecated]/4: member calls and member
// accesses (`m.f()`, `p->f()`, `m.operator-(1)`, a static member named
// through an object, a call through a pointer to member) are diagnosed
// like calls by name; a nodiscard class returned by value counts too.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 42:14 warning "'d' is deprecated"
//@ diag: 33:5 note "'d' has been explicitly marked deprecated here"
//@ diag: 42:25 warning "'d' is deprecated"
//@ diag: 33:5 note "'d' has been explicitly marked deprecated here"
//@ diag: 42:30 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 47:5 warning "'d' is deprecated"
//@ diag: 33:5 note "'d' has been explicitly marked deprecated here"
//@ diag: 47:13 warning "'d' is deprecated"
//@ diag: 33:5 note "'d' has been explicitly marked deprecated here"
//@ diag: 47:20 warning "'o' is deprecated: x"
//@ diag: 35:5 note "'o' has been explicitly marked deprecated here"
//@ diag: 47:35 warning "'f' is deprecated"
//@ diag: 36:5 note "'f' has been explicitly marked deprecated here"
//@ diag: 47:40 warning "'sd' is deprecated"
//@ diag: 37:5 note "'sd' has been explicitly marked deprecated here"
//@ diag: 48:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 48:10 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 48:18 warning "ignoring return value of function declared with 'nodiscard' attribute: why"
//@ diag: 48:26 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 48:42 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 48:49 warning "ignoring return value of type 'N' declared with 'nodiscard' attribute"
//@ diag: 49:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 50:3 warning "expression result unused; should this cast be to 'void'?"
//@ requires: clang>=22
// clang 22 spelling of member-call.cpp (WP-377).
struct M {
  [[deprecated]] void d();
  void o(int);
  [[deprecated("x")]] void o(long);
  [[deprecated]] int f;
  [[deprecated]] static void sd();
  [[nodiscard]] int m();
  [[nodiscard("why")]] static int sm();
  [[nodiscard]] int operator-(int);
  [[nodiscard]] M &operator+(int);
  void g() { d(); this->d(); m(); }
};
struct [[nodiscard]] N {};
struct Q { N n(); N &r(); };
void h(M m, M *p, Q q) {
  m.d(); p->d(); m.o(1L); (void)m.f; m.sd();
  m.m(); p->m(); m.sm(); m.operator-(1); m + 1; q.n(); q.r();
  (m.*(&M::m))();
  (void*)p;
}
