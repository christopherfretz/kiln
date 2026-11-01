// [expr.const], [class.union] (WP-213): the "in call to" note names a member
// operator call's object by its first operand as written (clang
// `describeCall`: `u.s.data[0].operator=(e)`, `*p.operator+=(0)`), and a
// reference argument into a union by the member it was formed through
// (`u.s.data[0]`, not the first member `u.i` at the same offset).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-division-by-zero
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:48 note "division by zero"
//@ diag: 30:46 note "in call to 'u.s.data[0].operator=(e)'"
//@ diag: 31:15 note "in call to 'asg()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:38 note "division by zero"
//@ diag: 32:67 note "in call to '*p.operator+=(0)'"
//@ diag: 33:15 note "in call to 'cpd()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:43 note "division by zero"
//@ diag: 34:43 note "in call to 'r.f(0)'"
//@ diag: 35:45 note "in call to 'by_ref(u.s.data[0])'"
//@ diag: 36:15 note "in call to 'ref()'"
struct E {
  int v;
  constexpr E& operator=(const E& o) { v = o.v / 0; return *this; }
  constexpr E& operator+=(int x) { v /= x; return *this; }
  constexpr int f(int x) const { return v / x; }
};
struct S { E data[2]; };
union UU { int i; S s; };
constexpr int asg() { UU u{.s = {}}; E e{1}; u.s.data[0] = e; return 0; }
static_assert(asg() == 0);
constexpr int cpd() { UU u{.s = {}}; E* p = &u.s.data[0]; return (*p += 0).v; }
static_assert(cpd() == 0);
constexpr int by_ref(const E& r) { return r.f(0); }
constexpr int ref() { UU u{.s = {}}; return by_ref(u.s.data[0]); }
static_assert(ref() == 0);
