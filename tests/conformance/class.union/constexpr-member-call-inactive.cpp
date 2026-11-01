// [class.union.general]/6, [expr.const]/10.9, [class.mfct.non.static]/2
// (WP-224): a member function call on a union member other than the active
// one is not a constant expression (clang's `checkDynamicType` with
// `AK_MemberCall`, reported at the member name or the operator). Only the
// operator syntax of a trivial assignment activates the member
// (`u.s = o`); the explicit `u.s.operator=(o)` is a member call on `s`.
// S's assignment is not trivial (S is polymorphic), so `x.s = S{2}` through
// an anonymous union is a member call too. Calls on the active member and
// on a member being constructed are fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:55 note "member call on member 'q' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 50:15 note "in call to 'explicit_trivial()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:51 note "member call on member 's' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 54:15 note "in call to 'call_inactive()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:50 note "member call on member 't' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'user_assign()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:59 note "member call on member 't' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 58:15 note "in call to 'user_assign_explicit()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:65 note "member call on member 's' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 60:15 note "in call to 'through_pointer()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:45 note "member call on member 's' of union with no active member is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'no_active()'"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:54 note "member call on member 's' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 64:15 note "in call to 'virtual_inactive()'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:49 note "member call on member 's' of union with active member 'n' is not allowed in a constant expression"
//@ diag: 74:15 note "in call to 'anonymous_inactive()'"
struct S {
  int a;
  constexpr S(int x = 0) : a(x) {}
  constexpr int get() const { return a; }
  virtual constexpr int vget() const { return a + 1; }
};
struct T { int a; constexpr T &operator=(const T &o) { a = o.a; return *this; } };
struct Q { int a; };
union U { int i; S s; };
union V { int i; T t; };
union W { int i; Q q; };
union E { int i; S s; constexpr E() {} constexpr ~E() {} };

constexpr int explicit_trivial() { W w{}; Q o{1}; w.q.operator=(o); return w.q.a; }
static_assert(explicit_trivial() == 1);
constexpr int operator_trivial() { W w{}; Q o{1}; w.q = o; return w.q.a; }
static_assert(operator_trivial() == 1);
constexpr int call_inactive() { U u{}; return u.s.get(); }
static_assert(call_inactive() == 0);
constexpr int user_assign() { V v{}; T o{1}; v.t = o; return v.t.a; }
static_assert(user_assign() == 1);
constexpr int user_assign_explicit() { V v{}; T o{1}; v.t.operator=(o); return v.t.a; }
static_assert(user_assign_explicit() == 1);
constexpr int through_pointer() { U u{}; S *p = &u.s; return p->get(); }
static_assert(through_pointer() == 0);
constexpr int no_active() { E e; return e.s.get(); }
static_assert(no_active() == 0);
constexpr int virtual_inactive() { U u{}; return u.s.vget(); }
static_assert(virtual_inactive() == 0);

constexpr int active() { U u{.s = S{4}}; return u.s.get() + u.s.vget(); }
static_assert(active() == 9);
struct C { int a; constexpr C(int x) : a(x) { a = get() * 2; } constexpr int get() const { return a; } };
union UC { int i; C c; constexpr UC() : c(3) {} };
constexpr int during_construction() { UC u; return u.c.get(); }
static_assert(during_construction() == 6);
struct A { union { int n; S s; }; };
constexpr int anonymous_inactive() { A x{}; x.s = S{2}; return x.s.get(); }
static_assert(anonymous_inactive() == 2);
struct R { int a; constexpr int get() const { return a; } };
struct A2 { union { int n; R r; }; };
constexpr int anonymous_active() { A2 y{}; y.r = R{2}; return y.r.get(); }
static_assert(anonymous_active() == 2);
