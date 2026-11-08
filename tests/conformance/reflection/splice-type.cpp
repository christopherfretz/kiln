// [dcl.type.splice] P2996R13: `typename [: r :]` designates the type a
// constant reflection r reflects; in a template argument list it must be
// written `typename [: r :]` ([temp.names]/1). The operand is a
// converted constant expression of type std::meta::info
// ([basic.splice]/2), so any constant expression producing one works.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
using namespace std::meta;

constexpr info ri = ^^int;
typename [: ri :] x = 1;
typename [: ^^double :] y = 2.5;
static_assert(__is_same(decltype(x), int) && __is_same(decltype(y), double));

template <class T>
struct Box {
  T v;
};
Box<typename [: ^^long :]> b;
static_assert(__is_same(decltype(b.v), long));
static_assert(__is_same(Box<typename [: ri :]>, Box<int>));

struct S {
  int a;
};
typename [: type_of(^^S::a) :] z = 0;
static_assert(__is_same(decltype(z), int));

consteval info pick(bool c) { return c ? ^^char : ^^short; }
typename [: pick(true) :] c = 'a';
typename [: pick(false) :] s = 1;
static_assert(sizeof(c) == 1 && sizeof(s) == 2);

using T2 = typename [: ^^const float :];
static_assert(__is_same(T2, const float));

int f() {
  typename [: ^^S :] local{3};
  return local.a;
}
