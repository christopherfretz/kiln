// [expr.const]/14, [dcl.constexpr]/6 (WP-231): clang rejects a constant
// result referring to a lifetime-extended temporary of destructed type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:20 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 9:24 note "non-trivial destruction of lifetime-extended temporary with type 'const D' used in the result of a constant expression is not yet supported"
struct D { int a; constexpr ~D() {} };
// A destructed temporary cannot be part of a constant result.
constexpr const D& r = D{1};
struct T { int a; };
constexpr const T& t = T{1};
static_assert(t.a == 1);
