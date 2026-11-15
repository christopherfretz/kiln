// [meta.reflection.array], [meta.reflection.substitute],
// [meta.reflection.define.aggregate]: a range argument is any input
// range, read as a range-based for statement reads it (spec/reflection.md
// §3.5): a filtered or `take_while` view, a user's input range and a
// user's container are iterated, once (one `begin`, one `*` and one `++`
// per element), never read from their layout.
//@ args: -stdlib=libstdc++ -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#include <iterator>
#include <ranges>
#ifdef __KILN__
#include "kiln_meta.h"
#define REFLECT_ARRAY(...) std::meta::__kiln_reflect_constant_array(__VA_ARGS__)
#else
#include <meta>
#define REFLECT_ARRAY(...) std::meta::reflect_constant_array(__VA_ARGS__)
#endif
using namespace std::meta;

constexpr int v[]{1, 2, 3, 4, 5, 6};

constexpr info evens = REFLECT_ARRAY(v | std::views::filter([](int i) { return i % 2 == 0; }));
static_assert(type_of(evens) == ^^const int[3]);
static_assert(extract<const int*>(evens)[2] == 6);

constexpr info low = REFLECT_ARRAY(v | std::views::take_while([](int i) { return i < 3; }));
static_assert(type_of(low) == ^^const int[2]);

// A single-pass range counting what is called on it.
struct Counts {
  int begins = 0, derefs = 0, incs = 0;
};
struct Once {
  int n;
  Counts* c;
  struct It {
    const Once* o;
    int i;
    using value_type = int;
    using difference_type = int;
    constexpr int operator*() const {
      ++o->c->derefs;
      return 10 * i;
    }
    constexpr It& operator++() {
      ++o->c->incs;
      ++i;
      return *this;
    }
    constexpr void operator++(int) { ++*this; }
    constexpr bool operator==(std::default_sentinel_t) const { return i == o->n; }
  };
  constexpr It begin() const {
    ++c->begins;
    return {this, 0};
  }
  constexpr std::default_sentinel_t end() const { return {}; }
};
static_assert(std::ranges::input_range<Once>);

consteval bool single_pass() {
  Counts c;
  info r = REFLECT_ARRAY(Once{4, &c});
  return type_of(r) == ^^const int[4] && extract<const int*>(r)[3] == 30 && c.begins == 1 &&
         c.derefs == 4 && c.incs == 4;
}
static_assert(single_pass());

// A user's container of reflections: substitute iterates it.
template <class T, class U> struct Pair {};
struct Args {
  info a[2];
  consteval const info* begin() const { return a; }
  consteval const info* end() const { return a + 2; }
};
static_assert(substitute(^^Pair, Args{{^^int, ^^char}}) == ^^Pair<int, char>);
static_assert(can_substitute(^^Pair, Args{{^^int, ^^char}}));
