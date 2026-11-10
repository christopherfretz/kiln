// [meta.reflection.member.queries]/3 (P2996R13, C++26 DIS): a function or
// variable whose type contains an undeduced placeholder type is not
// members-of-representable, so members_of omits `auto spin();` until a
// definition deduces it; a member template, a member defined in its
// class and one defined before the query are reported.
// [meta.reflection.queries]: has-type(r) is false for such a function, so
// type_of and return_type_of throw (is_const and the ref-qualifier
// queries are false); parameters_of and is_noexcept have no such
// condition. A member of a class template specialization whose
// definition is not instantiated stays undeduced (reflecting it
// instantiates nothing). The answers depend on the program point, so a
// consteval call asked before and after the definition is not memoized.
// Compared with GCC 16.1 (spec/reflection-queries.md §1). WP-1243.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta_queries.h"
#else
#include <meta>
#endif
using namespace std::meta;

consteval bool eq(std::string_view a, const char* b) {
  decltype(sizeof 0) n = 0;
  while (b[n] && n < a.size() && a.data()[n] == b[n]) ++n;
  return !b[n] && n == a.size();
}
consteval int count(info c, const char* nm) {
  int n = 0;
  for (info m : members_of(c, access_context::unchecked()))
    if (has_identifier(m) && eq(identifier_of(m), nm)) ++n;
  return n;
}
consteval bool throws_type(info r) {
  try { (void)type_of(r); return false; } catch (exception&) { return true; }
}
consteval bool throws_ret(info r) {
  try { (void)return_type_of(r); return false; } catch (exception&) { return true; }
}
consteval bool throws_params(info r) {
  try { (void)parameters_of(r); return false; } catch (exception&) { return true; }
}

// The reference workload's gate: 10 * functions reported + 1 when type_of
// of `spin` succeeds, 2 when it throws. 10: spin is not reported.
struct Gizmo {
  auto spin();
  int plain();
  template <class T> auto tm(T);
  auto inl() { return 1; }
  static auto sv();
};
consteval int code() {
  int n = 0, st = 0;
  for (info m : members_of(^^Gizmo, access_context::unchecked()))
    if (is_function(m) && !is_special_member_function(m) && !is_static_member(m) &&
        eq(identifier_of(m), "spin") + eq(identifier_of(m), "plain")) {
      ++n;
      if (eq(identifier_of(m), "spin")) st = throws_type(m) ? 2 : 1;
    }
  return n * 10 + st;
}
static_assert(code() == 10);
static_assert(count(^^Gizmo, "spin") == 0);
static_assert(count(^^Gizmo, "sv") == 0);
static_assert(count(^^Gizmo, "tm") == 1);
static_assert(count(^^Gizmo, "inl") == 1);
static_assert(throws_type(^^Gizmo::spin));
static_assert(throws_ret(^^Gizmo::spin));
static_assert(throws_type(^^Gizmo::sv));
static_assert(!throws_params(^^Gizmo::spin));
static_assert(parameters_of(^^Gizmo::spin).size() == 0);
static_assert(!is_noexcept(^^Gizmo::spin));
static_assert(type_of(^^Gizmo::inl) == ^^int());
static_assert(return_type_of(^^Gizmo::inl) == ^^int);

// Defined before the query point: deduced, reported, typed.
auto Gizmo::spin() { return 2L; }
static_assert(code() == 21);
static_assert(count(^^Gizmo, "spin") == 1);
static_assert(!throws_type(^^Gizmo::spin));
static_assert(type_of(^^Gizmo::spin) == ^^long());
static_assert(return_type_of(^^Gizmo::spin) == ^^long);

// Namespace members and a non-member function.
auto nsf();
namespace N {
auto f();
int g();
}
static_assert(count(^^N, "f") == 0);
static_assert(count(^^N, "g") == 1);
static_assert(throws_type(^^nsf));
static_assert(throws_type(^^N::f));
namespace N {
auto f() { return 'c'; }
}
static_assert(count(^^N, "f") == 1);
static_assert(type_of(^^N::f) == ^^char());

// A variable is deduced from its initializer's type before the
// initializer is evaluated: it is reported from inside it.
namespace V {
int a = 0;
constexpr auto b = count(^^V, "b");
}
static_assert(V::b == 1);

// is_const / is_*_reference_qualified read type_of when has-type holds.
struct Q {
  auto c() const;
  auto r() &&;
  int d() const;
};
static_assert(!is_const(^^Q::c));
static_assert(!is_rvalue_reference_qualified(^^Q::r));
static_assert(is_const(^^Q::d));

// A class template specialization: an `auto` member defined in the
// template is deduced only when its definition is instantiated.
template <class T> struct S {
  auto f() { return T{}; }
  auto g();
};
static_assert(count(^^S<int>, "f") == 0);
static_assert(count(^^S<int>, "g") == 0);
static_assert(throws_type(^^S<int>::f));
static_assert(throws_ret(^^S<int>::g));

int main() {}
