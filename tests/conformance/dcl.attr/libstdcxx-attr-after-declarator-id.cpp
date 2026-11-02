// [dcl.decl.general]/4 (noptr-declarator: declarator-id
// attribute-specifier-seq(opt)), [dcl.attr.grammar]/5: an attribute between
// the declarator-id and the parameter list appertains to the entity, here
// for an operator-function-id and a member function template, as in
// libstdc++ <compare>:549 (`operator() [[nodiscard]] (_Tp&& __t, ...)`) and
// the ranges function objects (docs/status/libstdcxx.md, WP-188). Verified
// against clang 19.
//@ expect: compile-ok
struct S {
  constexpr int operator() [[nodiscard]] (int x) const { return x; }
  template<typename T>
  constexpr T g [[nodiscard]] (T t) const { return t; }
  template<typename T>
    requires (sizeof(T) > 0)
  constexpr T operator[] [[nodiscard]] (T t) const { return t; }
};
constexpr int h [[nodiscard]] (int x) { return x + 1; }
static_assert(S{}(3) == 3);
static_assert(S{}.g(4) == 4);
static_assert(S{}[5] == 5);
static_assert(h(1) == 2);
