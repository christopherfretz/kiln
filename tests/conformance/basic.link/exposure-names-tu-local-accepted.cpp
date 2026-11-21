// [basic.link]/15-17: what naming a TU-local entity in a module interface
// does NOT expose. The body of a non-inline function, the initializer of a
// non-inline variable, a type alias, a friend declaration, a local class,
// a constant-initialized const variable's value (also in sizeof), a
// constant read in an inline function (not an odr-use, /14.4), an
// unexported declaration of a TU-local entity itself, and an inline
// function naming a function from the global module fragment. GCC 16.1
// accepts all of it (warning for the last); clang 22 accepts it, warning
// on the last. An unevaluated operand names what it names (WP-1821):
// exposure-unevaluated-discarded.cpp; so does a discarded-value
// expression or a full-expression that folds (WP-1847):
// exposure-full-expression-folded.cpp.
//@ expect: compile-ok
//@ std: c++20
module;
static int gmf_helper() { return 2; }
export module M;
static int helper() { return 1; }
static int target;
static const int K = 4;
namespace {
struct S { int v; };
enum class E { a, b };
}

export int plain() { return helper() + target; }
export int plain_var = helper();
using Alias = S;
export struct F {
  friend int helper_friend(S);
};
static int also_local(S s) { return s.v; }
inline int unexported_inline_uses_nothing() { return 0; }
export inline int reads_constant() { return K; }
export inline int sizes() { return sizeof(K) + sizeof(int); }
export inline int local_class() {
  struct L { int f() { return 0; } };
  return L{}.f();
}
export inline int discarded_branch() {
  if constexpr (false)
    return 0;
  return 1;
}
export inline int from_gmf() { return gmf_helper(); }
export int use_alias() { Alias a{1}; return a.v + also_local(a) + int(E::b); }
