// [expr.const.reflect] (CWG3162), [module.reach], [basic.def.point]: the
// point of evaluation of a constexpr local's initializer is where it
// appears, so a class whose definition (here an injected declaration
// after a consteval block) follows that point is incomplete there,
// whichever evaluation later calls the function; one whose definition
// precedes it is complete. Only the initializer is frozen at its point:
// the same query in the function's body is evaluated with the call, and
// a member function called on the materialized object runs then too.
// GCC 16.1 agrees (spec/oven.md "Constexpr locals", WP-1673).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
using std::meta::info;

struct S;
consteval bool frozen() { constexpr bool c = std::meta::is_complete_type(^^S); return c; }
consteval bool live() { return std::meta::is_complete_type(^^S); }
struct Box {
  info r;
  consteval bool complete() const { return std::meta::is_complete_type(r); }
};
consteval bool via_object() { constexpr Box b{^^S}; return b.complete(); }

// Two levels: inner's point precedes the completion, outer's follows it.
struct T;
consteval bool inner() { constexpr bool c = std::meta::is_complete_type(^^T); return c; }

consteval {
  std::meta::define_aggregate(^^S, {});
  std::meta::define_aggregate(^^T, {});
  // Inside the evaluation that completed S: the body's query sees it, the
  // constexpr local's initializer (point in frozen's body) does not.
  if (!live()) throw 1;
  if (frozen()) throw 2;
  if (!via_object()) throw 3;
}

consteval bool outer() { constexpr bool c = inner(); return c; }
consteval bool after() { constexpr bool c = std::meta::is_complete_type(^^T); return c; }

// Outside it: the same answers.
static_assert(live());
static_assert(!frozen());
static_assert(via_object());
static_assert(!inner());
static_assert(!outer());
static_assert(after());

int main() {}
