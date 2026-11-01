// [expr.const]/4, /5.9: a trivial copy reads every member, so copying an
// object with a mutable member (or a union whose active member is mutable)
// whose lifetime began outside the evaluation is not a constant expression.
// The read carries the trivial copy constructor's "in call to" frame.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:13 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 20:17 note "read of mutable member 'm' is not allowed in a constant expression"
//@ diag: 20:17 note "in call to 'S(s)'"
//@ diag: 15:31 note "declared here"
//@ diag: 22:13 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 22:17 note "read of mutable member 'b' is not allowed in a constant expression"
//@ diag: 22:17 note "in call to 'U(u)'"
//@ diag: 16:30 note "declared here"
struct S { int k; mutable int m; };
union U { int a; mutable int b; };
struct E {};
struct T { int k; mutable E e; };
constexpr S s{1, 2};
constexpr S c = s;
constexpr U u{.b = 3};
constexpr U d = u;
constexpr T t{4, {}};
constexpr T t2 = t;
static_assert(t2.k == 4);
