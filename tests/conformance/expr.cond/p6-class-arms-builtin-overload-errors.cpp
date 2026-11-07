// [expr.cond]/6: overload resolution over the built-in `operator?:`
// candidates that finds no best candidate makes the expression ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:14 error "conditional expression is ambiguous; 'A' and 'B' can be converted to several common types"
//@ diag: 14:14 error "incompatible operand types ('A' and 'C')"
struct A { operator int() const; operator double() const; };
struct B { operator long() const; operator float() const; };
struct C {};
struct D { operator void*() const; };

void f(bool c) {
  auto x = c ? A{} : B{};
  auto y = c ? A{} : C{};
  auto z = c ? D{} : nullptr;
}
