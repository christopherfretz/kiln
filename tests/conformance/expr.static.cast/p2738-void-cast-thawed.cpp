// [expr.static.cast]/14, [expr.const]/5.14 (C++26, P2738R1): a `void *`
// constant thawed back into constant evaluation keeps the type of the
// object it designates (the `designated` type of the frozen pointer,
// spec/oven.md "Construction"), so a cast to a different type at the
// same address is diagnosed as clang does: an empty base at offset 0 is
// neither the complete object, nor the first member, nor a
// `[[no_unique_address]]` member sharing its address.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'E' is not similar to the target type 'const int'"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'E' is not similar to the target type 'const S'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'E' is not similar to the target type 'const F'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'F' is not similar to the target type 'const E'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'const E'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:45 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'const S'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 75:16 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'const D' is not similar to the target type 'const int'"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'const D'"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'const D' is not similar to the target type 'const int'"
//@ diag: 82:15 error "static assertion expression is not an integral constant expression"
//@ diag: 82:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'const int' is not similar to the target type 'const D'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:15 note "cast from 'CVP' (aka 'const void *') is not allowed in a constant expression because the pointed object type 'D' is not similar to the target type 'CI' (aka 'const int')"
//@ diag: 91:15 error "static assertion expression is not an integral constant expression"
//@ diag: 91:15 note "cast from 'const void *' is not allowed in a constant expression because the pointed object type 'const int' is not similar to the target type 'const D'"
struct E {};
struct F {};
// `E` (a base), `f` (a `[[no_unique_address]]` member) and `x` all live
// at offset 0 of an `S`.
struct S : E { [[no_unique_address]] F f; int x; };
struct D { int a; const int c; };
struct W { D d; };
using CVP = const void *;

constexpr S s{};
constexpr D d{1, 2};
constexpr D darr[2] = {{1, 2}, {3, 4}};
constexpr W w{{5, 6}};

// The right type: the complete object, a member, an array element.
constexpr CVP gs = &s;
constexpr CVP gd = &d;
constexpr CVP ga = &d.a;
constexpr CVP gel = &darr[1];
static_assert(static_cast<const S *>(gs)->x == 0);
static_assert(static_cast<const D *>(gd)->c == 2);
static_assert(*static_cast<const int *>(ga) == 1);
static_assert(static_cast<const D *>(gel)->a == 3);

// The empty base at offset 0 is not the first member, nor the complete
// object, nor the `[[no_unique_address]]` member sharing its address.
constexpr CVP ge = static_cast<const E *>(&s);
static_assert(static_cast<const E *>(ge) != nullptr);
static_assert(static_cast<const int *>(ge) != nullptr);
static_assert(static_cast<const S *>(ge) != nullptr);
static_assert(static_cast<const F *>(ge) != nullptr);
constexpr CVP gf = &s.f;
static_assert(static_cast<const F *>(gf) != nullptr);
static_assert(static_cast<const E *>(gf) != nullptr);
constexpr CVP gx = &s.x;
static_assert(*static_cast<const int *>(gx) == 0);
static_assert(static_cast<const E *>(gx) != nullptr);
static_assert(static_cast<const S *>(gs) == static_cast<const S *>(gx));

// The complete object is not its first member, and back.
static_assert(*static_cast<const int *>(gd) == 1);
static_assert(static_cast<const D *>(ga)->c == 2);

// The pointed object type carries the qualifiers of the declaration that
// names it: the variable (through array elements) or the member.
static_assert(static_cast<const int *>(gel) != nullptr);
constexpr CVP gc = &darr[1].c;
static_assert(static_cast<const D *>(gc) != nullptr);
constexpr CVP gw = &w.d;
using CI = const int;
static_assert(static_cast<CI *>(gw) != nullptr);

// A pointer member of a constant keeps its designator too.
struct H { const void *p; };
constexpr H h{&d.c};
static_assert(*static_cast<const int *>(h.p) == 2);
static_assert(static_cast<const D *>(h.p) != nullptr);
