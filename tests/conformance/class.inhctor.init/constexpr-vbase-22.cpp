// A class with a virtual base is not a literal type ([basic.types.general]/10),
// so its inheriting constructors are not constexpr-suitable: a constexpr
// variable of it (or an array of it) is rejected with clang's
// `RequireLiteralType` notes, every virtual base listed in vbases() order,
// and creating one in a constexpr function is not a constant expression
// (clang `CheckLiteralType`), also through a virtual base that itself
// inherits its constructor.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
// Clang 22 twin of constexpr-vbase.cpp (P2448: no literal type check in C++23).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 32:13 error "constexpr variable cannot have non-literal type 'const M'"
//@ diag: 30:8 note "struct with virtual base class is not a literal type"
//@ diag: 30:12 note "virtual base class declared here"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:23 note "constructor inherited from base class 'V' cannot be used in a constant expression; derived class cannot be implicitly initialized"
//@ diag: 34:15 note "in call to 'f()'"
//@ diag: 30:33 note "declared here"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:23 note "constructor inherited from base class 'V' cannot be used in a constant expression; derived class cannot be implicitly initialized"
//@ diag: 37:15 note "in call to 'g()'"
//@ diag: 35:33 note "declared here"
//@ diag: 39:13 error "constexpr variable cannot have non-literal type 'const W[1]'"
//@ diag: 38:7 note "class with virtual base classes is not a literal type"
//@ diag: 30:12 note "virtual base class declared here"
//@ diag: 38:21 note "virtual base class declared here"
struct V { constexpr V(int v) : v(v) {} int v; };
struct M : virtual V { using V::V; };
struct N : V { using V::V; };
constexpr M m(1);
constexpr int f() { M x(2); return x.v; }
static_assert(f() == 2);
struct P : virtual N { using N::N; };
constexpr int g() { P p(3); return p.v; }
static_assert(g() == 3);
class W : public M, virtual public N { public: W() : V(0), M(0), N(0) {} };
constexpr W w[1] = {};
M dyn(4);
