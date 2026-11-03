// [meta.reflection.queries] P2996R13: is_complete_type and
// is_enumerable_type, with the IS example. Both answer for the point of
// evaluation: a class is complete after its closing brace, and
// enumerable there or in one of its complete-class contexts (a member
// function body), not elsewhere in its member-specification; an opaque
// enumeration declaration is complete (its underlying type is fixed)
// but enumerable only after its enumerator-list. Neither is memoized
// (spec/reflection.md §3.3: PointDependent), so the same call answers
// false and then true. is_complete_type of a class template
// specialization that could be implicitly instantiated instantiates it.
// Answers checked against GCC 16.1 (-freflection).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

namespace ex {
// [meta.reflection.queries], is_enumerable_type's example.
class S;
enum class E;
static_assert(!is_enumerable_type(^^S));
static_assert(!is_enumerable_type(^^E));

class S {
  void mfn() {
    static_assert(is_enumerable_type(^^S));
  }
  static_assert(!is_enumerable_type(^^S));
};
static_assert(is_enumerable_type(^^S));

enum class E {
  A = is_enumerable_type(^^E) ? 1 : 2
};
static_assert(is_enumerable_type(^^E));
static_assert(static_cast<int>(E::A) == 2);
}

struct Inc;
consteval bool complete_inc() { return is_complete_type(^^Inc); }
static_assert(!complete_inc() && !is_complete_type(^^Inc));
struct Inc { int x; };
static_assert(complete_inc() && is_complete_type(^^Inc));

enum class Opaque : int;
static_assert(is_complete_type(^^Opaque) && !is_enumerable_type(^^Opaque));
enum Plain { p };
static_assert(is_complete_type(^^Plain) && is_enumerable_type(^^Plain));

template <class T> struct X { T t; };
static_assert(is_complete_type(^^X<int>));  // instantiated here
template <class T> struct Undef;
static_assert(!is_complete_type(^^Undef<int>));

using A = Inc;
static_assert(is_complete_type(^^A) && is_complete_type(^^const Inc));
static_assert(is_complete_type(^^int) && !is_complete_type(^^void));
static_assert(is_complete_type(^^int[2]) && !is_complete_type(^^int[]));
static_assert(!is_enumerable_type(^^int) && !is_enumerable_type(^^int[2]));
static_assert(!is_complete_type(^^Inc::x) && !is_complete_type(info{}));
