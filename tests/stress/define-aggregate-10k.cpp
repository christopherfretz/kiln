// [meta.reflection.define.aggregate] (P2996R13), spec/reflection.md §6.2:
// one consteval block defines a class with 10,000 data members (names
// m0000..m9999, built in a constexpr buffer), and later evaluations read
// its layout and one object of it. The name check is one hash set over the
// 10,000 names; the specs are interned by content (§6.1), so the block's
// peak is its own buffers and the 10,000 infos.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: evaluations<=10 peak_bytes<=140000 allocs<=1
//@ oven: 32 steps<=170000 bytes<=1200000 peak_bytes<=140000 allocs<=0
#include "kiln_meta.h"
using namespace std::meta;

constexpr int N = 10000;
struct Big;

consteval void define_big() {
  char names[N][5] = {};
  info specs[N] = {};
  for (int i = 0; i < N; ++i) {
    names[i][0] = 'm';
    names[i][1] = char('0' + i / 1000);
    names[i][2] = char('0' + i / 100 % 10);
    names[i][3] = char('0' + i / 10 % 10);
    names[i][4] = char('0' + i % 10);
    specs[i] = data_member_spec(^^int, {.name = std::string_view(names[i], 5)});
  }
  define_aggregate(^^Big, specs);
}
consteval { define_big(); }

static_assert(sizeof(Big) == N * sizeof(int));
static_assert(__builtin_offsetof(Big, m9999) == (N - 1) * sizeof(int));
constexpr int last() {
  Big* b = new Big{};
  b->m9999 = 7;
  b->m0000 = 5;
  int r = b->m9999 + b->m0000 + b->m4242;
  delete b;
  return r;
}
static_assert(last() == 12);
