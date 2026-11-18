// [expr.reflect]/1, /7, [expr.prim.id.qual]/1, [expr.prim.lambda.closure]/1:
// a computed type-specifier (`decltype(e)`, a pack-index-specifier) as the
// scope of a qualified-id may be followed by an operator-function-id, a
// conversion-function-id, a destructor name or a template-id, and the
// operand is then an id-expression, never a type-id. This names a
// closure's function call operator, which has no other spelling:
// `^^decltype(a)::operator()`. A generic lambda's call operator is a
// function template (as `^^G::operator()` for a class G whose only
// `operator()` is a template, GCC 16.1 agrees). Found running GCC's C++
// testsuite; GCC 16.1 accepts all of it.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
#include <meta>
using namespace std::meta;

auto a = [] { return 1; };
constexpr info r = ^^decltype(a)::operator();
static_assert(is_function(r) && !is_function_template(r));
static_assert(is_operator_function(r) && operator_of(r) == op_parentheses);
static_assert(parent_of(r) == ^^decltype(a));
static_assert(!has_identifier(r));
static_assert(type_of(r) == ^^int() const);
static_assert(a.[:r:]() == 1);

using FP = int (*)();
constexpr info conv = ^^decltype(a)::operator FP;
static_assert(is_conversion_function(conv) && parent_of(conv) == ^^decltype(a));

auto gl = [](auto x) { return x; };
constexpr info g = ^^decltype(gl)::operator();
static_assert(is_function_template(g));
constexpr info gi = ^^decltype(gl)::operator()<int>;
static_assert(is_function(gi) && template_of(gi) == g);

struct L {
  int operator()() const { return 2; }
  constexpr ~L() {}
  constexpr operator int() const { return 3; }
  template <class U> static constexpr int v = 4;
  template <class U> void h(U) {}
};
L l;
static_assert(^^decltype(l)::~L == ^^L::~L);
static_assert(is_destructor(^^decltype(l)::~L));
static_assert(^^decltype(l)::operator int == ^^L::operator int);
static_assert(^^decltype(l)::template h<int> == ^^L::h<int>);
static_assert(^^decltype(l)::v<int> == ^^L::v<int>);

// Dependent: resolved at instantiation.
template <class T> consteval info call_of(T t) { return ^^decltype(t)::operator(); }
template <class... Ts> consteval info first_call() { return ^^Ts...[0]::operator(); }
template <class... Ts> consteval info first_dtor() { return ^^Ts...[0]::~L; }
template <class... Ts> consteval info first_conv() { return ^^Ts...[0]::operator int; }
static_assert(call_of(L{}) == ^^L::operator());
static_assert(call_of(a) == r);
static_assert(first_call<decltype(a), int>() == r);
static_assert(first_dtor<L>() == ^^L::~L);
static_assert(first_conv<L, int>() == ^^L::operator int);

int main() { return a.[:r:]() - 1; }
