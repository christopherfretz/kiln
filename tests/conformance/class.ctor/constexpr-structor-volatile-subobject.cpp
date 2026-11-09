// [class.ctor.general]/5, [class.dtor]/5, [expr.const]/10.8: volatile
// semantics are lifted only for the object a constructor or destructor is
// running for. A base subobject at offset 0 shares its complete object's
// address but not its identity: re-constructing or destroying it does not
// make the rest of a volatile complete object accessible. Invalid twin of
// constexpr-structor-volatile-subobject-ok.cpp. WP-1101.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:13 note "assignment to volatile object 'd' is not allowed in a constant expression"
//@ diag: 41:3 note "in call to 'construct_at<B, volatile D *, bool>(&d, &d, true)'"
//@ diag: 44:15 note "in call to 'write_in_ctor()'"
//@ diag: 40:14 note "volatile object declared here"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:13 note "assignment to volatile object 'd' is not allowed in a constant expression"
//@ diag: 47:3 note "in call to 'destroy_at<B>(&d)'"
//@ diag: 50:15 note "in call to 'read_in_dtor()'"
//@ diag: 46:14 note "volatile object declared here"
#include <memory>
struct D;
struct B {
  D *self;
  constexpr B(volatile D *p, bool write);
  constexpr ~B();
};
struct D : B {
  int m;
  constexpr D(bool keep) : B(keep ? this : nullptr, false), m(1) {}
};
constexpr B::B(volatile D *p, bool write) : self(const_cast<D *>(p)) {
  if (write)
    self->m = 2;
}
constexpr B::~B() {
  if (self)
    self->m += 1;
}
constexpr int write_in_ctor() {
  volatile D d(false);
  std::construct_at(static_cast<B *>(const_cast<D *>(&d)), &d, true);
  return 0;
}
static_assert(write_in_ctor() == 0);
constexpr int read_in_dtor() {
  volatile D d(true);
  std::destroy_at(static_cast<B *>(const_cast<D *>(&d)));
  return 0;
}
static_assert(read_in_dtor() == 0);
