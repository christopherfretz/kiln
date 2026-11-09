// [meta.reflection.queries] P2996R13: parent_of, has_parent and dealias,
// with the IS examples. parent_of(r) is the innermost class, function
// or namespace whose scope is the target scope of r's declaration (an
// anonymous union for its members, the enumeration for an enumerator,
// the function for a parameter); has_parent is false for the global
// namespace, an entity with C language linkage, and a type that is
// neither a class nor an enumeration nor an alias. dealias strips every
// level of alias and returns anything else unchanged. Answers checked
// against GCC 16.1 (-freflection). A `^^T &&` parses as `^^(T&&)`,
// hence the parentheses.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
#include "kiln_meta_traits.h"
using namespace std::meta;

// [meta.reflection.queries], parent_of's example.
struct I { };

struct F : I {
  union {
    int o;
  };

  enum N {
    A
  };
};

constexpr auto ctx = std::meta::access_context::current();

static_assert(parent_of(^^F) == ^^::);
static_assert(parent_of(bases_of(^^F, ctx)[0]) == ^^F);
static_assert(is_union_type(parent_of(^^F::o)));
static_assert(parent_of(^^F::N) == ^^F);
static_assert(parent_of(^^F::A) == ^^F::N);
// The same, without is_union_type:
static_assert(is_type(parent_of(^^F::o)) && parent_of(^^F::o) != ^^F);
static_assert(parent_of(parent_of(^^F::o)) == ^^F);

// [meta.reflection.queries], dealias's example.
using X = int;
using Y = X;
static_assert(dealias(^^int) == ^^int);
static_assert(dealias(^^X) == ^^int);
static_assert(dealias(^^Y) == ^^int);

namespace Ns { int nv; namespace In { struct T {}; } }
namespace NA = Ns;
namespace NB = NA;
enum class EC { a };
template <class T> struct CT {};
template <class T> using AT = CT<T>;
struct S { using MA = long; int m; void f(); };
extern "C" int cvar;
void fn() {}

static_assert(parent_of(^^Ns::nv) == ^^Ns && parent_of(^^Ns::In::T) == ^^Ns::In);
static_assert(parent_of(^^Ns) == ^^:: && parent_of(^^NA) == ^^::);
static_assert(parent_of(^^EC::a) == (^^EC) && parent_of(^^S::MA) == (^^S));
static_assert(parent_of(^^S::m) == (^^S) && parent_of(^^S::f) == ^^S);
static_assert(parent_of(^^X) == ^^:: && parent_of(^^CT) == ^^::);
static_assert(parent_of(^^CT<int>) == ^^:: && parent_of(^^fn) == ^^::);
static_assert(has_parent(^^X) && has_parent(^^EC) && has_parent(^^EC::a));
static_assert(!has_parent(^^::) && !has_parent(^^cvar));
static_assert(!has_parent(^^int) && !has_parent(^^int*) && !has_parent(^^void()));
static_assert(!has_parent(info{}));

void h() {
  int loc = 0;
  struct L { int lm; };
  static_assert(parent_of(^^loc) == ^^h && parent_of(^^L) == ^^h);
  static_assert(parent_of(^^L::lm) == ^^L);
}

static_assert(dealias(^^NA) == ^^Ns && dealias(^^NB) == ^^Ns && dealias(^^Ns) == ^^Ns);
static_assert(dealias(^^AT<int>) == (^^CT<int>) && dealias(^^S::MA) == ^^long);
static_assert(dealias(^^S::m) == ^^S::m && dealias(^^CT) == ^^CT);
static_assert(dealias(info{}) == info{});
