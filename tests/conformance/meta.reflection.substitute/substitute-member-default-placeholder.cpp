// [meta.reflection.substitute], [dcl.spec.auto.general],
// [temp.deduct.general], [meta.reflection.names] (P2996R13): a
// member function template with a default template argument and a
// decltype(auto) return type. Substituting arguments for which the body
// compiles instantiates it and deduces the return type, and the
// reflection then behaves as the deduced specialization: type_of,
// return_type_of, is_function, a splice call, extract and reflect_constant
// of the pointer to member; it has no identifier (a function template
// specialization), and display_string_of is implementation-defined. A
// check in the declaration (a SFINAE default template argument, a trailing
// return type, a constraint) fails in the immediate context, so
// can_substitute is false and the body is never instantiated, also with
// an empty argument list that uses the defaults. Reduced from real code.
// GCC 16.1 answers true for a member template without deducing its
// return type (docs/status/deviations.md, WP-1674, WP-1929).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not deduce a member function template specialization's return type in substitute (WP-1674)
//@ strict-diags
#include <meta>
#include <type_traits>
#include <vector>
using namespace std::meta;

struct box {
  template <typename E = void>
  constexpr decltype(auto) put() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return E{7};
  }
  template <typename E = void, typename = std::enable_if_t<!std::is_void_v<E>>>
  constexpr decltype(auto) guarded() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return 1;
  }
  template <typename E = void>
  constexpr auto trailing() -> decltype(E::value) {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return E::value;
  }
  template <typename E = void>
    requires(!std::is_void_v<E>)
  constexpr decltype(auto) constrained() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return 2;
  }
};
struct has_value { static constexpr int value = 3; };

constexpr info r = substitute(^^box::put, {^^int});
static_assert(can_substitute(^^box::put, {^^int}));
static_assert(is_function(r) && !is_function_template(r));
static_assert(type_of(r) == ^^int());
static_assert(return_type_of(r) == ^^int);
static_assert(!has_identifier(r));
static_assert(!display_string_of(r).empty());
static_assert(template_of(r) == ^^box::put);
static_assert(box{}.[:r:]() == 7);
constexpr auto pm = extract<int (box::*)()>(r);
static_assert((box{}.*pm)() == 7);
static_assert((box{}.*[:reflect_constant(pm):])() == 7);

consteval bool probe(info t) { return can_substitute(t, std::vector<info>{}); }
static_assert(!probe(^^box::guarded));
static_assert(!probe(^^box::trailing));
static_assert(!probe(^^box::constrained));
static_assert(can_substitute(^^box::guarded, {^^int}));
static_assert(return_type_of(substitute(^^box::trailing, {^^has_value})) == ^^const int);
static_assert(box{}.[:substitute(^^box::constrained, {^^long}):]() == 2);

int main() {}
