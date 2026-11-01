// [class.temporary]/6, [expr.const]/14 (clang CheckLValueConstantExpression):
// a lifetime-extended temporary whose type has a non-trivial destructor
// (of the type, an element or a member) cannot be referred to by the result
// of a constant initializer; a trivially destructible one, a temporary
// destroyed within the evaluation, and a dynamic initializer are fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:14 error "constexpr variable 'rd' must be initialized by a constant expression"
//@ diag: 25:17 note "non-trivial destruction of lifetime-extended temporary with type 'const D' used in the result of a constant expression is not yet supported"
//@ diag: 26:20 error "constexpr variable 'd1' must be initialized by a constant expression"
//@ diag: 26:25 note "non-trivial destruction of lifetime-extended temporary with type 'const D' used in the result of a constant expression is not yet supported"
//@ diag: 27:15 error "constexpr variable 'd2' must be initialized by a constant expression"
//@ diag: 27:20 note "non-trivial destruction of lifetime-extended temporary with type 'D' used in the result of a constant expression is not yet supported"
//@ diag: 32:14 error "constexpr variable 'ra' must be initialized by a constant expression"
//@ diag: 32:17 note "non-trivial destruction of lifetime-extended temporary with type 'const D[2]' used in the result of a constant expression is not yet supported"
//@ diag: 34:20 error "constexpr variable 'e1' must be initialized by a constant expression"
//@ diag: 34:25 note "non-trivial destruction of lifetime-extended temporary with type 'const E' used in the result of a constant expression is not yet supported"
//@ diag: 36:29 error "constexpr variable 'd3' must be initialized by a constant expression"
//@ diag: 36:34 note "non-trivial destruction of lifetime-extended temporary with type 'const D' used in the result of a constant expression is not yet supported"
//@ diag: 40:20 error "variable does not have a constant initializer"
//@ diag: 40:1 note "required by 'constinit' specifier here"
//@ diag: 40:25 note "non-trivial destruction of lifetime-extended temporary with type 'const D' used in the result of a constant expression is not yet supported"
struct D { int v; constexpr ~D() {} };
struct RD { const D& r; };
constexpr RD rd{{1}};
constexpr const D& d1 = {2};
constexpr D&& d2 = D{3};
struct T { int v; };
struct RT { const T& r; };
constexpr RT rt{{1}};
struct RA { const D (&a)[2]; };
constexpr RA ra{{{1}, {2}}};
struct E { D d; };
constexpr const E& e1 = {{4}};
void f() {
  static constexpr const D& d3 = {5};
  constexpr int k = RD{{1}}.r.v;
}
const D& d4 = {6};
constinit const D& d5 = {7};
struct W { const D& r; int x; };
constexpr int g() { W w{{1}, 2}; return w.r.v; }
static_assert(g() == 1);
