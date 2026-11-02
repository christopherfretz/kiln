// A class with a virtual base is not a literal type ([basic.types.general]/10),
// so its inheriting constructors are not constexpr-suitable: a constexpr
// variable of it (or an array of it) is rejected with clang's
// `RequireLiteralType` notes, every virtual base listed in vbases() order,
// and creating one in a constexpr function is not a constant expression
// (clang `CheckLiteralType`), also through a virtual base that itself
// inherits its constructor.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:13 error "constexpr variable cannot have non-literal type 'const M'"
//@ diag: 26:8 note "struct with virtual base class is not a literal type"
//@ diag: 26:12 note "virtual base class declared here"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:23 note "non-literal type 'M' cannot be used in a constant expression"
//@ diag: 30:15 note "in call to 'f()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:23 note "non-literal type 'P' cannot be used in a constant expression"
//@ diag: 33:15 note "in call to 'g()'"
//@ diag: 35:13 error "constexpr variable cannot have non-literal type 'const W[1]'"
//@ diag: 34:7 note "class with virtual base classes is not a literal type"
//@ diag: 26:12 note "virtual base class declared here"
//@ diag: 34:21 note "virtual base class declared here"
//@ requires: clang<22
// WP-388 (vbase construction): clang 22 notes the inherited constructor of a virtual base, not the non-literal type.
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
