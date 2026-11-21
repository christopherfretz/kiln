// Local classes and closures of functions that are not inline, naming
// TU-local entities where no deduced return type names them
// (exposure-local-class-importer-run.cpp; the module-only checks are
// basic.link/exposure-local-class-not-inline-accepted.cpp).
export module xlcl;
static int hidden = 7;
static int helper(int x) { return x + 1; }
namespace { struct S { int v; }; }

export int constexpr_member() { struct L { constexpr int *g() { return &hidden; } }; return *L().g(); }
export int inline_member() { struct L { inline int g(int x) { return helper(x); } }; return L().g(1); }
export int declarations() { struct L : S { S s; int g(S t) { return t.v; } }; return L{{1}, {2}}.g(S{3}); }
export int called_lambda(int x) { return [=] { return helper(x); }(); }
export auto not_returned() { struct L { S s; int v = 2; }; return L().v; }
export auto member_body() { struct L { int g(int x) { return helper(x); } }; return L(); }
