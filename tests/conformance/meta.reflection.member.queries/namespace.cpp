// [meta.reflection.member.queries]/1-4 (P2996R13): members_of a namespace
// lists the members declared so far, at the point of the query, in
// declaration order: variables, functions, classes, enumerations,
// templates, type aliases and nested (inline) namespaces, each once
// however often redeclared (including through a linkage specification).
// The global namespace ^^:: is accepted. The answer grows with the
// program (the query is PointDependent, spec/reflection.md §3.3).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr auto all = access_context::unchecked();
consteval unsigned long count(info ns) { return members_of(ns, all).size(); }
consteval info nth(info ns, unsigned long i) { return members_of(ns, all)[i]; }

namespace N {
int v;
void f();
void f();
struct C;
enum E { e };
template <class T>
struct Tm {};
using A = int;
namespace inner {}
inline namespace in {}
}  // namespace N
static_assert(count(^^N) == 8);
static_assert(nth(^^N, 0) == ^^N::v);
static_assert(nth(^^N, 1) == ^^N::f);
static_assert(nth(^^N, 2) == ^^N::C);
static_assert(nth(^^N, 3) == ^^N::E);
static_assert(nth(^^N, 4) == ^^N::Tm);
static_assert(nth(^^N, 5) == ^^N::A);
static_assert(nth(^^N, 6) == ^^N::inner);
static_assert(nth(^^N, 7) == ^^N::in);

// The namespace reopened: the new members follow; a redeclaration of an
// earlier one adds nothing.
namespace N {
int w;
void f();
extern "C++" {
int x;
}
}  // namespace N
static_assert(count(^^N) == 10);
static_assert(nth(^^N, 8) == ^^N::w);
static_assert(nth(^^N, 9) == ^^N::x);

// A function's later definition is not a second member.
void N::f() {}
static_assert(count(^^N) == 10);

// An empty namespace, and the global namespace.
namespace Empty {}
static_assert(count(^^Empty) == 0);
void last_global();
static_assert(nth(^^::, count(^^::) - 1) == ^^last_global);

int main() {}
