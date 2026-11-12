// [meta.reflection.define.aggregate] P2996R13 (spike vehicle
// `__kiln_define_member`): a constant evaluation in the member-specification
// of the class being defined adds a non-static data member at that point.
// Lookup, layout, aggregate initialization and members_of see it.
//@ expect: compile-ok
//@ args: -Wno-kiln-extension -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
// -fconstexpr-report attributes each defining evaluation to its line
// (one per specialization for the templates); nothing stays allocated.
//@ oven: 26 evaluations<=1 steps<=8 allocs<=0
//@ oven: 59 evaluations<=5 steps<=10
//@ oven: 76 evaluations<=1 steps<=256 peak_bytes<=640 refl_bytes<=600 allocs<=0
//@ oven: 84 evaluations<=2
#include "kiln_meta.h"
using namespace std::meta;

consteval bool define(info cls, info type, const char* name) {
  return __kiln_define_member(cls, type, name) != info{};
}

// [class.mem]: the member lands where the evaluation happens, so `c`
// follows it; a later member function body finds it ([class.mem]/7).
struct S {
  int a;
  static_assert(define(^^S, ^^long, "injected"));
  char c;
  constexpr long get() const { return injected; }
};
static_assert(sizeof(S) == 24);
static_assert(__builtin_offsetof(S, injected) == 8);
static_assert(__builtin_offsetof(S, c) == 16);
static_assert(nonstatic_data_members_of(^^S, access_context::unchecked()).size() == 3);
static_assert(name_is(members_of(^^S, access_context::unchecked())[1], "injected"));
static_assert(type_of(members_of(^^S, access_context::unchecked())[1]) == ^^long);
constexpr S s{1, 2, 'c'};
static_assert(s.injected == 2 && s.get() == 2 && s.c == 'c');
constexpr long bump() { S x{1, 41, 'x'}; x.injected += 1; return x.get(); }
static_assert(bump() == 42);

// `^^F` names the first declaration; the definition is the class being
// defined. A pointer to F interned while F was incomplete stays usable.
struct F;
consteval bool null_f(const F* p) { return p == nullptr; }
struct F {
  int a;
  static_assert(null_f(nullptr));
  static_assert(define(^^F, ^^short, "h"));
};
static_assert(sizeof(F) == 8 && __builtin_offsetof(F, h) == 4);
constexpr F f{1, 2};
static_assert(f.h == 2 && !null_f(&f));

// [temp.inst]: each specialization runs its own evaluation, with its own
// argument; a member function uses `this->` for the injected member
// (unqualified lookup at definition time would not find it, [temp.res]).
template <class T> struct G {
  char tag;
  static_assert(define(^^G, ^^T, "v"));
  constexpr T get() const { return this->v; }
};
static_assert(sizeof(G<char>) == 2 && sizeof(G<long>) == 16);
static_assert(G<int>{'a', 7}.get() == 7 && G<double>{'b', 1.5}.get() == 1.5);
static_assert(members_of(^^G<short>, access_context::unchecked()).size() == 9);

// Several members from one evaluation, names built at compile time (a
// member function would not be defined yet: [class.mem]/7).
consteval bool fill(info cls) {
  for (int i = 0; i < 20; ++i) {
    char n[4] = {'m', char('0' + i / 10), char('0' + i % 10), 0};
    if (!define(cls, ^^int, n)) return false;
  }
  return true;
}
struct Many {
  static_assert(fill(^^Many));
};
static_assert(sizeof(Many) == 80 && __builtin_offsetof(Many, m19) == 76);

// On-demand completion ([temp.inst]/2): W<short> completes as a member of
// Outer, W<double> under sizeof in a constant expression; each order is
// the order of the demands.
template <class T> struct W {
  static_assert(define(^^W, ^^T, "w"));
};
struct Outer { W<short> x; int y; };
static_assert(sizeof(Outer) == 8 && __builtin_offsetof(Outer, y) == 4);
static_assert(sizeof(W<double>) == 8);
static_assert(name_is(members_of(^^W<double>, access_context::unchecked())[0], "w"));
