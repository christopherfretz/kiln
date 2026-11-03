// [class.base.init]/9, [expr.const]/10.8 (P3533R2): a virtual base the most
// derived constructor does not name is default-initialized; its lifetime has
// begun, so reading an indeterminate member is an uninitialized read (as for
// a non-virtual base in clang), a default member initializer runs, and a
// write before the read is fine (spec/oven.md "Unconstructed bases").
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:76 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 18:33 note "in call to 'd.get()'"
//@ diag: 19:15 note "in call to 'f()'"
struct V { int x; };
struct W { int y = 3; };

struct D : virtual V { constexpr D() {} constexpr int get() const { return x; } };
constexpr int f() { D d; return d.get(); }
static_assert(f() == 0);

struct D2 : virtual W { constexpr D2() {} };
constexpr int g() { D2 d; return d.y; }
static_assert(g() == 3);

struct D3 : virtual V { constexpr D3() {} };
constexpr int h() { D3 d; d.x = 4; return d.x; }
static_assert(h() == 4);
