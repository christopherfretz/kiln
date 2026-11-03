// [meta.reflection.define.aggregate]/10 (P2996R13): the class is complete
// as soon as define_aggregate returns, in the same evaluation: a later
// description in it may have the class's type, a later call may use the
// class as a member, and its members are visible. A pointer to the class
// made while it was incomplete is then a pointer to a complete type
// (spec/reflection.md §6.2: the Oven's provisional slot is refreshed).
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

struct P;
struct Q;
struct W;
template <class T> consteval unsigned long count_after(info cls, T* p) {
  if (p != nullptr) throw 0;
  define_aggregate(cls, {data_member_spec(^^int, {.name = "x"}),
                         data_member_spec(^^int, {.name = "y"})});
  return __kiln_members_count(cls);
}
consteval {
  P* before = nullptr;
  if (count_after(^^P, before) != 2) throw 1;
  // P is complete: a description of type P and a class with a P member.
  define_aggregate(^^Q, {data_member_spec(^^P, {.name = "p"}),
                         data_member_spec(^^P*, {.name = "next"})});
  define_aggregate(^^W, {data_member_spec(^^Q, {.name = "q"})});
  if (__kiln_members_count(^^W) != 1) throw 2;
}
static_assert(sizeof(P) == 2 * sizeof(int));
static_assert(sizeof(Q) == sizeof(P) + sizeof(P*));
static_assert(sizeof(W) == sizeof(Q));
constexpr W w{{{1, 2}, nullptr}};
static_assert(w.q.p.x == 1 && w.q.p.y == 2 && w.q.next == nullptr);

// Members injected into one class are seen by the next consteval block.
struct V;
consteval { define_aggregate(^^V, {data_member_spec(^^char, {.name = "c"})}); }
consteval {
  if (__kiln_members_count(^^V) != 1) throw 3;
}

// An Oven type slot made for Z while it was incomplete serves Z's objects
// once a later evaluation completed it.
struct Z;
consteval {
  Z* z = nullptr;
  if (z != nullptr) throw 4;
}
consteval {
  define_aggregate(^^Z, {data_member_spec(^^long, {.name = "n"}), data_member_spec(^^Z*, {.name = "self"})});
}
consteval {
  Z* z = new Z{5, nullptr};
  z->self = z;
  long n = z->self->n;
  delete z;
  if (n != 5) throw 5;
}
