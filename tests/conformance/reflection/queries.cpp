// [meta.reflection.queries], [meta.reflection.member.queries] P2996R13:
// type_of, name_of (identifier_of; the spike returns a const char * into
// a frozen string literal object) and members_of (the spike: a
// fixed-capacity member_list built from __kiln_members_count /
// __kiln_member_at), all answered by sema through SemaBridge while the
// Oven runs. members_of lists the members in declaration order
// ([class.mem.general]/1) once the class is complete, even when the
// consteval function asking was written before the class was defined.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
using namespace std::meta;

struct S;
consteval unsigned long count_s() { return __kiln_members_count(^^S); }
consteval info last_of_s() { return __kiln_member_at(^^S, count_s() - 1); }

struct S {
  int a;
  double b;
  static int c;
  void f();
  char d;
};

// Declaration order, including the static data member and the function;
// implicitly declared special members are not listed.
static_assert(count_s() == 5);
static_assert(__kiln_member_at(^^S, 0) == ^^S::a);
static_assert(__kiln_member_at(^^S, 2) == ^^S::c);
static_assert(last_of_s() == ^^S::d);
constexpr auto m = members_of<^^S>();
static_assert(m.size() == 5 && m[1] == ^^S::b && m[3] == ^^S::f);

// type_of: variables, data members, functions.
int x;
static_assert(type_of(^^x) == ^^int);
static_assert(type_of(^^S::b) == ^^double);
static_assert(type_of(^^S::c) == ^^int);
static_assert(type_of(m[0]) == ^^int);
int g(char);
static_assert(type_of(^^g) == ^^int(char));

// name_of: declarations and class types.
static_assert(name_is(^^S::b, "b"));
static_assert(name_is(m[4], "d"));
static_assert(name_is(^^S, "S"));
static_assert(name_is(^^g, "g"));
namespace N {}
static_assert(name_is(^^N, "N"));
constexpr const char* nm = name_of(^^x);
static_assert(nm[0] == 'x' && nm[1] == 0);

// A class template specialization is completed (instantiated) on demand.
template <class T>
struct W {
  T first;
  int second;
};
static_assert(__kiln_members_count(^^W<char>) == 2);
static_assert(type_of(__kiln_member_at(^^W<char>, 0)) == ^^char);
static_assert(type_of(members_of<^^W<long>>()[0]) == ^^long);
