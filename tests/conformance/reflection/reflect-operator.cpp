// [expr.reflect] P2996R13: `^^type-id`, `^^namespace-name` and
// `^^id-expression` for a variable, a non-overloaded function and a data
// member. Redeclarations reflect the same entity; types reflect their
// canonical type in the spike (spec/sema.md "Reflection (spike,
// -freflection)").
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
using info = decltype(^^int);

namespace N {
int v;
extern int v;
void f();
void f();
struct S { int a; int b; };
namespace Inner {}
}
namespace M = N;

static_assert(^^N == ^^N);
static_assert(^^M != ^^N); // [expr.reflect]/5: the alias itself
static_assert(^^N::Inner != ^^N);
static_assert(^^N::v == ^^N::v);
static_assert(^^N::f == ^^N::f);
static_assert(^^N::f != ^^N::v);
static_assert(^^N::S::a != ^^N::S::b);
static_assert(^^N::S != ^^N::S::a);
static_assert(^^N::S == ^^N::S);
static_assert(^^int* != ^^int);
static_assert(^^int[3] != ^^int[4]);
static_assert(^^void (*)(int) == ^^void (*)(int));

using namespace N;
static_assert(^^v == ^^N::v);

template <class T> constexpr info refl = ^^T;
static_assert(refl<int> == ^^int);
static_assert(refl<N::S> == ^^N::S);
static_assert(refl<int> != refl<long>);

template <class T> struct Box { static constexpr info elem = ^^T*; };
static_assert(Box<char>::elem == ^^char*);
