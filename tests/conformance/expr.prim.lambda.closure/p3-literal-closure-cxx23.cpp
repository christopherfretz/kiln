// [dcl.constexpr]/3 as amended by P2448R2 (C++23): a constexpr function's
// return and parameter types need not be literal, so a closure capturing a
// non-literal type by copy is accepted where C++17/20 reject it
// (p3-literal-closure-cxx20.cpp). WP-1225.
//@ std: c++23
//@ strict-diags

struct NL { NL(); ~NL(); int v; };
constexpr auto copy_nl(NL n) { return [n] { return n.v; }; }
constexpr auto init_nl(NL &n) { return [c = n] { return c.v; }; }
struct S {
  NL n;
  constexpr auto self_copy() { return [*this] { return 1; }; }
};
inline auto f(NL n) { return [n] { return n.v; }; }
constexpr int take(decltype(f(NL())) l) { return 1; }

// A reference to a non-literal type is literal in every mode.
constexpr auto ref_nl(NL &n) { return [&n] { return n.v; }; }
constexpr auto init_ref_nl(NL &n) { return [&r = n] { return r.v; }; }
