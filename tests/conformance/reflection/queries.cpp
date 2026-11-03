// [meta.reflection.queries], [meta.reflection.member.queries] P2996R13:
// type_of, name_of (identifier_of; the spike returns a const char * into
// a frozen string literal object) and members_of, all answered by sema
// through SemaBridge while the Oven runs. members_of lists the members in
// declaration order ([class.mem.general]/1), then the implicitly declared
// special members, once the class is complete, even when the consteval
// function asking was written before the class was defined.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
using namespace std::meta;

struct S;
consteval info m(unsigned long i) { return members_of(^^S, access_context::unchecked())[i]; }
consteval unsigned long count_s() { return members_of(^^S, access_context::unchecked()).size(); }

struct S {
  int a;
  double b;
  static int c;
  void f();
  char d;
};

// Declaration order, including the static data member and the function;
// the six implicitly declared special members follow.
static_assert(count_s() == 11);
static_assert(m(0) == ^^S::a);
static_assert(m(2) == ^^S::c);
static_assert(m(4) == ^^S::d);
static_assert(m(1) == ^^S::b && m(3) == ^^S::f);

// type_of: variables, data members, functions.
int x;
static_assert(type_of(^^x) == ^^int);
static_assert(type_of(^^S::b) == ^^double);
static_assert(type_of(^^S::c) == ^^int);
static_assert(type_of(m(0)) == ^^int);
int g(char);
static_assert(type_of(^^g) == ^^int(char));

// name_of: declarations and class types.
static_assert(name_is(^^S::b, "b"));
static_assert(name_is(m(4), "d"));
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
static_assert(nonstatic_data_members_of(^^W<char>, access_context::unchecked()).size() == 2);
static_assert(type_of(nonstatic_data_members_of(^^W<char>, access_context::unchecked())[0]) == ^^char);
static_assert(type_of(members_of(^^W<long>, access_context::unchecked())[0]) == ^^long);
