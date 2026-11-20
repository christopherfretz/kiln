// [expr.cond]/1: the first operand of GNU `x ?: y` is contextually
// converted to `bool`; a class without a conversion makes it ill-formed,
// and [expr.cond]/4-6 still reject arms with no conversion between them.
// WP-1738 (GAP-052).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:10 error "value of type 'C' is not contextually convertible to 'bool'"
//@ diag: 14:12 error "incompatible operand types ('int' and 'C')"
//@ diag: 15:12 error "incompatible operand types ('B' and 'C')"
struct C { int v; };
struct B { explicit operator bool() const; };
void f(C c, int k, B b) {
  (void)(c ?: k);
  (void)(k ?: c);
  (void)(b ?: c);
}
