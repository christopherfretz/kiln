// [expr.const]/10, [dcl.init.aggr]/4: an initializer that keeps a prior
// initializer and re-initializes part of it (a DesignatedInitUpdateExpr)
// is not evaluated as a constant expression (clang has no evaluation rule
// for it either), so a constexpr variable so initialized is ill-formed,
// diagnosed at the kept initializer; a non-constexpr one is initialized at
// run time.
//@ expect: compile-fail
//@ args: -Wno-c99-designator -Wno-initializer-overrides
//@ strict-diags
//@ diag: 18:23 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:15 note "initializer of 'b' is not a constant expression"
//@ diag: 18:13 note "declared here"
//@ diag: 21:23 error "constexpr variable 'c' must be initialized by a constant expression"
struct A { int x, y; };
struct B { A a; int z; };
struct C { char s[4]; };
constexpr B b = {.a = A{1, 2}, .a.y = 3};
static_assert(b.a.x == 1 && b.a.y == 3);
B ok = {.a = A{1, 2}, .a.y = 3};
constexpr C c = {.s = "ab", .s[0] = 'z'};
