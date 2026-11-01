// [dcl.constexpr]/3 (C++23, P2448R2): a constexpr function need not be able
// to produce a constant expression, and its return and parameter types need
// not be literal types; only an actual constant evaluation is checked.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
int runtime();
struct NonLiteral { NonLiteral(); ~NonLiteral(); };
constexpr int never_constant() { return runtime(); }
constexpr NonLiteral make() { return NonLiteral(); }
constexpr void take(NonLiteral) {}
constexpr int ok(bool b) { return b ? 1 : runtime(); }
static_assert(ok(true) == 1);
