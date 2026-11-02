// [expr.static.cast]/6: static_cast<void>(e) discards e on purpose; its
// result is never reported unused, while a bare comparison or a comma's
// unused left operand is (libc++ tuple size_incompatible_comparison).
//@ strict-diags
//@ diag: 14:5 warning "equality comparison result unused"
//@ diag: 14:5 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 15:5 warning "equality comparison result unused"
//@ diag: 15:5 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 16:32 warning "expression result unused"
struct S { bool operator==(const S &) const; };
void f(int a, int b, S s) {
  static_cast<void>(a == b);
  (void)(a == b);
  a == b;
  s == s;
  static_cast<void>(s == s), a == b;
}
