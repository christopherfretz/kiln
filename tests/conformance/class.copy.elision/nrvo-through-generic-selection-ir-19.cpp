// The clang 19 twin of nrvo-through-generic-selection-ir.cpp (same program; the IR differs between the releases).
// [class.copy.elision]/1.1: a return operand that is a (possibly
// parenthesized) name of a local of the return type is a copy-elision
// candidate; clang strips the operand with `Expr::IgnoreParens`, so a
// resolved `_Generic` or `__builtin_choose_expr` around the name keeps it
// one, and the local is constructed in the return slot with no copy or
// move (WP-1176). C++23: C++26 kiln fills the local with its erroneous
// value ([basic.indet]/1), which clang 22 does not.
//@ expect: ir
//@ requires: clang<22
//@ std: c++23
struct S {
  S();
  S(const S &);
  S(S &&);
  ~S();
};
S generic() {
  S s;
  return _Generic(0, int: s, default: 0);
}
S choose() {
  S s;
  return (__builtin_choose_expr(0, 0, s));
}
