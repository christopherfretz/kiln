// [expr.mptr.oper]/4, /6: a null pointer to member, an object without the
// member, and a null object expression are not constant expressions; clang
// reports the member pointer operand, or the use of the null object.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:67 note "subexpression not valid in a constant expression"
//@ diag: 29:15 note "in call to 'null_member()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:80 note "subexpression not valid in a constant expression"
//@ diag: 31:15 note "in call to 'null_function()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:91 note "subexpression not valid in a constant expression"
//@ diag: 33:15 note "in call to 'no_such_member()'"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34 note "null pointer"
//@ diag: 35:15 note "in call to 'null_read()'"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36 note "null pointer"
//@ diag: 37:15 note "in call to 'null_write()'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:87 note "cannot access base class of null pointer"
//@ diag: 39:15 note "in call to 'null_base()'"
// clang 19 and 22 word and place these null-pointer notes differently; kiln
// follows its -fclang-compat level (WP-377), so the directives are column-less.
struct A { int a = 1; constexpr int fa() const { return a; } };
struct D : A { int d = 3; };
constexpr int null_member() { D x; int D::*p = nullptr; return x.*p; }
static_assert(null_member() == 0);
constexpr int null_function() { D x; int (D::*p)() const = nullptr; return (x.*p)(); }
static_assert(null_function() == 0);
constexpr int no_such_member() { A y; int A::*q = static_cast<int A::*>(&D::d); return y.*q; }
static_assert(no_such_member() == 0);
constexpr int null_read() { D *x = nullptr; int D::*p = &D::d; return x->*p; }
static_assert(null_read() == 0);
constexpr int null_write() { D *x = nullptr; int D::*p = &D::d; x->*p = 4; return 0; }
static_assert(null_write() == 0);
constexpr int null_base() { D *x = nullptr; int (D::*p)() const = &A::fa; return (x->*p)(); }
static_assert(null_base() == 0);
