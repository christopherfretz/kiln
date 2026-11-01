// [class.union.general]/6, [expr.const]/10.9 (WP-224): nested unions at the
// same offset each keep their own active member. `n.q.x = 1` activates `q`
// in `N2` and `x` in `q`; a read of `c`, of `q.y`, or of `q` after `c` is
// made active, is still diagnosed. A constant whose nested union's active
// member is not the first keeps it when read back.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:60 note "read of member 'c' of union with active member 'q' is not allowed in a constant expression"
//@ diag: 28:15 note "in call to 'outer_inactive()'"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:65 note "read of member 'y' of union with active member 'x' is not allowed in a constant expression"
//@ diag: 30:15 note "in call to 'inner_inactive()'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:66 note "read of member 'q' of union with active member 'c' is not allowed in a constant expression"
//@ diag: 32:15 note "in call to 'reactivated()'"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:15 note "read of member 'x' of union with active member 'y' is not allowed in a constant expression"
union N2 { char c; union { int x; float y; } q; };
constexpr int nested() { N2 n{}; n.q.x = 1; return n.q.x; }
static_assert(nested() == 1);
constexpr int nested_switch() { N2 n{}; n.q.x = 1; n.q.y = 2.0f; return (int)n.q.y; }
static_assert(nested_switch() == 2);
struct Wrap { union { long l; N2 n; } u; };
constexpr int deeper() { Wrap w{}; w.u.n.q.y = 3.0f; return (int)w.u.n.q.y; }
static_assert(deeper() == 3);
constexpr int outer_inactive() { N2 n{}; n.q.x = 1; return n.c; }
static_assert(outer_inactive() == 1);
constexpr int inner_inactive() { N2 n{}; n.q.x = 1; return (int)n.q.y; }
static_assert(inner_inactive() == 1);
constexpr int reactivated() { N2 n{}; n.q.x = 1; n.c = 2; return n.q.x; }
static_assert(reactivated() == 1);
constexpr N2 g = [] { N2 n{}; n.q.y = 3.0f; return n; }();
static_assert(g.q.y == 3.0f);
static_assert(g.q.x == 3);
