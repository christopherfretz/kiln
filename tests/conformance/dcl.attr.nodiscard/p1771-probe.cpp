// [dcl.attr.nodiscard]/2-4 (C++20, P1771R1 and P1301R4): `[[nodiscard]]`
// on a constructor warns when a temporary built by it is discarded; the
// optional string is printed. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 13:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: hold it"
struct Lock {
  [[nodiscard("hold it")]] Lock(int) {}
  Lock() {}
};
void f() {
  Lock(1);
  Lock();
  Lock kept(2);
  (void)Lock(3);
}
