// [meta.reflection.member.queries]/1-4 (P2996R13): members_of lists a
// complete class's members in declaration order, then the special members
// it declares implicitly (members_of forces their declaration; GCC 16.1's
// order: default constructor, copy constructor, copy assignment, move
// constructor, move assignment, destructor). A member alias is its alias,
// a nested class or enum its type, a member template its template; an
// unscoped enumerator, a static_assert and an injected-class-name are not
// members. The list is filtered by the access context: unchecked() keeps
// every member, unprivileged() the public ones, current() what the scope
// may name. A class template specialization is instantiated first.
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

struct S {
  int a;
  using T = int;
  struct N {};
  enum E { e0 };
  static_assert(true);
  template <class U>
  void tm(U) {}
  static int s;
  void f();
  int b;
};
consteval info nth(info r, unsigned long i) { return members_of(r, all)[i]; }
static_assert(members_of(^^S, all).size() == 8 + 6);
static_assert(nth(^^S, 0) == ^^S::a);
static_assert(nth(^^S, 1) == ^^S::T);
static_assert(nth(^^S, 2) == ^^S::N);
static_assert(nth(^^S, 3) == ^^S::E);
static_assert(nth(^^S, 4) == ^^S::tm);
static_assert(nth(^^S, 5) == ^^S::s);
static_assert(nth(^^S, 6) == ^^S::f);
static_assert(nth(^^S, 7) == ^^S::b);

// A user-declared special member is listed where it is declared; only the
// ones not declared are added (a user copy constructor suppresses the
// implicit moves, [class.copy.ctor]/8).
struct U {
  U(const U&);
  int x;
};
static_assert(members_of(^^U, all).size() == 1 + 1 + 2);
static_assert(nth(^^U, 1) == ^^U::x);

// Access filtering.
class P {
  int priv;
  void pf();

 protected:
  int prot;

 public:
  int pub;
  static consteval unsigned long inside() { return members_of(^^P, access_context::current()).size(); }
  friend consteval unsigned long friend_count();
};
consteval unsigned long friend_count() { return members_of(^^P, access_context::current()).size(); }
static_assert(members_of(^^P, all).size() == 5 + 6);
static_assert(members_of(^^P, access_context::unprivileged()).size() == 2 + 6);
static_assert(members_of(^^P, access_context::current()).size() == 2 + 6);
static_assert(P::inside() == 11);
static_assert(friend_count() == 11);
static_assert(members_of(^^P, access_context::unprivileged())[0] == ^^P::pub);

// A class template specialization: instantiated on demand.
template <class T>
struct W {
  T first;
  int second;
};
static_assert(type_of(members_of(^^W<long>, all)[0]) == ^^long);
static_assert(members_of(^^W<char>, all).size() == 2 + 6);

// A union and an empty class.
union Un {
  int i;
  float f;
};
static_assert(members_of(^^Un, all).size() == 2 + 6);
struct Empty {};
static_assert(members_of(^^Empty, all).size() == 6);

// Asked from a consteval function written before the class was complete:
// the answer is the complete class's.
struct Late;
consteval unsigned long late() { return members_of(^^Late, all).size(); }
struct Late {
  int a, b, c;
};
static_assert(late() == 3 + 6);

int main() {}
