// [dcl.pre]/P3289R1, [class.mem]: a consteval block is a
// member-declaration, evaluated when it is reached in the class body:
// the class is incomplete there and later members are not declared yet,
// so an injected member (`__kiln_define_member`, the spike vehicle of
// define_aggregate) lands at that point; a nested class's block runs
// before the enclosing class is complete.
//@ expect: compile-ok
//@ args: -Wno-kiln-extension -freflection -I tests/support
//@ skip(clang): clang 19 has no consteval blocks (P3289R1) or reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
struct S {
  int a;
  consteval { __kiln_define_member(^^S, ^^int, "injected"); }
  char c;
  consteval { __kiln_define_member(^^S, ^^int, "late"); }
};
static_assert(sizeof(S) == 4 * sizeof(int));
static_assert(__builtin_offsetof(S, injected) == sizeof(int));
static_assert(__builtin_offsetof(S, c) == 2 * sizeof(int));
static_assert(__builtin_offsetof(S, late) == 3 * sizeof(int));
constexpr S s{1, 2, 'c', 4};
static_assert(s.injected == 2 && s.late == 4);

struct Outer {
  struct Inner {
    consteval { __kiln_define_member(^^Inner, ^^char, "x"); }
  };
  consteval { __kiln_define_member(^^Outer, ^^Inner, "in"); }
  int tail;
};
static_assert(sizeof(Outer::Inner) == 1);
static_assert(__builtin_offsetof(Outer, tail) == sizeof(int));
static_assert(Outer{{'q'}, 3}.in.x == 'q');

// A local class and a union.
int local() {
  struct L {
    consteval { __kiln_define_member(^^L, ^^int, "v"); }
  };
  return L{7}.v;
}
union U {
  char c;
  consteval { __kiln_define_member(^^U, ^^int, "i"); }
};
static_assert(sizeof(U) == sizeof(int));
