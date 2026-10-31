//@ expect: compile-ok
// [expr.prim.req.general]/5: a requires-expression outside a template is
// evaluated when it appears.
struct S { int x; using type = int; };
static_assert(requires { 1 + 1; });
static_assert(requires(S s) { s.x; });
static_assert(requires { requires sizeof(int) == 4; });
static_assert(!requires { requires sizeof(int) == 3; });
constexpr bool b = requires { typename S::type; };
static_assert(b);
