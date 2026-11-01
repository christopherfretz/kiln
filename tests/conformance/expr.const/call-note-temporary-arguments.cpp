// [expr.const]/5, [class.temporary] (WP-241): a reference argument bound to
// a materialized temporary is printed in the "in call to" note as the
// temporary's expression as written (clang's APValue::printPretty of an
// expression base: `h(S{3, 'b'})`, `k(3)`, `h({3, 'b'})`), including a
// default argument and a lifetime-extended temporary.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:39 note "division by zero"
//@ diag: 46:15 note "in call to 'h({3, 'b'})'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:40 note "division by zero"
//@ diag: 47:15 note "in call to 'hc(3)'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:40 note "division by zero"
//@ diag: 48:15 note "in call to 'hc(C(3))'"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:39 note "division by zero"
//@ diag: 49:15 note "in call to 'h(make())'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:45 note "division by zero"
//@ diag: 50:15 note "in call to 'kd(3)'"
//@ diag: 51:17 error "cannot take the address of an rvalue of type 'int'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:52 note "division by zero"
//@ diag: 52:15 note "in call to 'hd(S{3, 'd'})'"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:53 note "division by zero"
//@ diag: 53:15 note "in call to 'two(S{1, 'x'}, make())'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:41 note "division by zero"
//@ diag: 54:15 note "in call to 'k(1 + 2)'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:39 note "division by zero"
//@ diag: 56:15 note "in call to 'h(S{3, 'e'})'"
struct S { int a; char b; };
struct C { int v; constexpr C(int x) : v(x) {} };
constexpr S make() { return S{3, 'c'}; }
constexpr int h(const S& s) { return 1/(s.a-3); }
constexpr int hc(const C& c) { return 1/(c.v-3); }
constexpr int k(const int& s) { return 1/(s-3); }
constexpr int kd(const double& s) { return 1/((int)s-3); }
constexpr int p(const int* q) { return 1/(*q-3); }
constexpr int hd(const S& s = S{3, 'd'}) { return 1/(s.a-3); }
constexpr int two(const S& s, const S& t) { return 1/(t.a-3); }
static_assert(h({3, 'b'}));
static_assert(hc(3));
static_assert(hc(C(3)));
static_assert(h(make()));
static_assert(kd(3));
static_assert(p(&make().a));
static_assert(hd());
static_assert(two(S{1,'x'}, make()));
static_assert(k(1+2));
constexpr const S& ext = S{3, 'e'};
static_assert(h(ext));
