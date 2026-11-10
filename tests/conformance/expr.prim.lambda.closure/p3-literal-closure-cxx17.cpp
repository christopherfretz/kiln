// [expr.prim.lambda.closure]/3 (P0170R1), [basic.types.general]/10,
// [dcl.constexpr]/3 (C++17): a closure type is literal only when its
// members are, so one capturing a non-literal type by copy (a simple, an
// init- or a `*this` capture) cannot be returned or taken by a constexpr
// function; a reference capture is literal. clang `RequireLiteralType` names
// the missing constexpr constructor first, as for any class (WP-1225).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 20:16 error "constexpr function's return type '(lambda at "
//@ diag: 20:39 note "is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 21:16 error "constexpr function's return type '(lambda at "
//@ diag: 21:40 note "is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 24:18 error "constexpr function's return type '(lambda at "
//@ diag: 24:39 note "is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 27:38 error "constexpr function's 1st parameter type 'decltype(f(NL()))' (aka '(lambda at "
//@ diag: 26:30 note "is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"

struct NL { NL(); ~NL(); int v; };
constexpr auto copy_nl(NL n) { return [n] { return n.v; }; }
constexpr auto init_nl(NL &n) { return [c = n] { return c.v; }; }
struct S {
  NL n;
  constexpr auto self_copy() { return [*this] { return 1; }; }
};
inline auto f(NL n) { return [n] { return n.v; }; }
constexpr int take(decltype(f(NL())) l) { return 1; }

// Valid: a reference to a non-literal type is literal.
constexpr auto ref_nl(NL &n) { return [&n] { return n.v; }; }
constexpr auto init_ref_nl(NL &n) { return [&r = n] { return r.v; }; }
