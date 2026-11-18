// [expr.const.reflect] (CWG3162): a constexpr variable's initializer is
// its own manifestly constant-evaluated evaluation, and its evaluation
// context holds only the injected declarations produced during it, so it
// does not see a class the enclosing evaluation completed with
// define_aggregate, at any depth of nesting; the enclosing evaluation
// sees the class from the completion on, before and after a nested one
// ran. The same function called directly and from a nested evaluation
// gets each its own answer (the call is not memoized across the two).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
struct S;
struct T;
consteval bool done(std::meta::info r) { return std::meta::is_complete_type(r); }
consteval int level2() {
  constexpr int a = done(^^S) ? 1 : 0;
  return a;
}
consteval int level1() {
  constexpr int b = level2() + (done(^^S) ? 10 : 0) + (std::meta::is_enumerable_type(^^S) ? 100 : 0);
  return b;
}
consteval bool nested_t() {
  constexpr bool c = std::meta::is_complete_type(^^T[2]);
  return c;
}
consteval {
  if (done(^^S) || level1() != 0) throw 1;
  std::meta::define_aggregate(^^S, {std::meta::data_member_spec(^^int, {.name = "x"})});
  if (!done(^^S)) throw 2;
  if (level1() != 0 || level2() != 0) throw 3;
  if (!done(^^S) || !std::meta::is_enumerable_type(^^S)) throw 4;
  if (std::meta::size_of(^^S) != sizeof(int)) throw 5;
  if (nested_t()) throw 6;
  // Completed after the nested evaluations above ran: seen from here on
  // by this evaluation, never by a nested one.
  std::meta::define_aggregate(^^T, {});
  if (!std::meta::is_complete_type(^^T[2]) || nested_t()) throw 7;
  if (std::meta::nonstatic_data_members_of(^^S, std::meta::access_context::unchecked())
          .size() != 1)
    throw 8;
}
