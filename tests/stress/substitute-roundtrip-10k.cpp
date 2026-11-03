// [meta.reflection.substitute] (P2996R13), docs/plans/m6-reflection.md
// §3.8 S5 without the ops other WPs own (static_data_members_of, WP-478;
// reflect_constant and extract, WP-479): 10,000 distinct substitute +
// template_arguments_of + template_of round trips, 100 per evaluation, each
// cell<K, prev> a new specialization that is never instantiated. Every
// template_arguments_of vector is reclaimed within its iteration, so the
// constexpr heap peak stays one vector (spec/landed/reflection/WP-482.md,
// "Performance").
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ oracle: gcc
//@ strict-diags
//@ oven: peak_bytes<=64 peak_arena<=64 allocs<=10000 evaluations<=210 steps<=160000
//@ mem<=1024
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif

template <class T, class U>
struct cell {
  using type = T;
};

template <int I> struct tag {};

constexpr std::meta::info kinds[] = {^^char, ^^short, ^^int, ^^long long};

consteval int chunk(std::meta::info seed) {
  int n = 0;
  std::meta::info prev = seed;
  for (int i = 0; i < 100; ++i) {
    auto r = std::meta::substitute(^^cell, {kinds[i % 4], prev});
    auto args = std::meta::template_arguments_of(r);
    if (args[0] != kinds[i % 4] || args[1] != prev || std::meta::template_of(r) != ^^cell) return -1;
    prev = r;
    ++n;
  }
  return n;
}
static_assert(chunk(^^tag<0>) == 100);
static_assert(chunk(^^tag<1>) == 100);
static_assert(chunk(^^tag<2>) == 100);
static_assert(chunk(^^tag<3>) == 100);
static_assert(chunk(^^tag<4>) == 100);
static_assert(chunk(^^tag<5>) == 100);
static_assert(chunk(^^tag<6>) == 100);
static_assert(chunk(^^tag<7>) == 100);
static_assert(chunk(^^tag<8>) == 100);
static_assert(chunk(^^tag<9>) == 100);
static_assert(chunk(^^tag<10>) == 100);
static_assert(chunk(^^tag<11>) == 100);
static_assert(chunk(^^tag<12>) == 100);
static_assert(chunk(^^tag<13>) == 100);
static_assert(chunk(^^tag<14>) == 100);
static_assert(chunk(^^tag<15>) == 100);
static_assert(chunk(^^tag<16>) == 100);
static_assert(chunk(^^tag<17>) == 100);
static_assert(chunk(^^tag<18>) == 100);
static_assert(chunk(^^tag<19>) == 100);
static_assert(chunk(^^tag<20>) == 100);
static_assert(chunk(^^tag<21>) == 100);
static_assert(chunk(^^tag<22>) == 100);
static_assert(chunk(^^tag<23>) == 100);
static_assert(chunk(^^tag<24>) == 100);
static_assert(chunk(^^tag<25>) == 100);
static_assert(chunk(^^tag<26>) == 100);
static_assert(chunk(^^tag<27>) == 100);
static_assert(chunk(^^tag<28>) == 100);
static_assert(chunk(^^tag<29>) == 100);
static_assert(chunk(^^tag<30>) == 100);
static_assert(chunk(^^tag<31>) == 100);
static_assert(chunk(^^tag<32>) == 100);
static_assert(chunk(^^tag<33>) == 100);
static_assert(chunk(^^tag<34>) == 100);
static_assert(chunk(^^tag<35>) == 100);
static_assert(chunk(^^tag<36>) == 100);
static_assert(chunk(^^tag<37>) == 100);
static_assert(chunk(^^tag<38>) == 100);
static_assert(chunk(^^tag<39>) == 100);
static_assert(chunk(^^tag<40>) == 100);
static_assert(chunk(^^tag<41>) == 100);
static_assert(chunk(^^tag<42>) == 100);
static_assert(chunk(^^tag<43>) == 100);
static_assert(chunk(^^tag<44>) == 100);
static_assert(chunk(^^tag<45>) == 100);
static_assert(chunk(^^tag<46>) == 100);
static_assert(chunk(^^tag<47>) == 100);
static_assert(chunk(^^tag<48>) == 100);
static_assert(chunk(^^tag<49>) == 100);
static_assert(chunk(^^tag<50>) == 100);
static_assert(chunk(^^tag<51>) == 100);
static_assert(chunk(^^tag<52>) == 100);
static_assert(chunk(^^tag<53>) == 100);
static_assert(chunk(^^tag<54>) == 100);
static_assert(chunk(^^tag<55>) == 100);
static_assert(chunk(^^tag<56>) == 100);
static_assert(chunk(^^tag<57>) == 100);
static_assert(chunk(^^tag<58>) == 100);
static_assert(chunk(^^tag<59>) == 100);
static_assert(chunk(^^tag<60>) == 100);
static_assert(chunk(^^tag<61>) == 100);
static_assert(chunk(^^tag<62>) == 100);
static_assert(chunk(^^tag<63>) == 100);
static_assert(chunk(^^tag<64>) == 100);
static_assert(chunk(^^tag<65>) == 100);
static_assert(chunk(^^tag<66>) == 100);
static_assert(chunk(^^tag<67>) == 100);
static_assert(chunk(^^tag<68>) == 100);
static_assert(chunk(^^tag<69>) == 100);
static_assert(chunk(^^tag<70>) == 100);
static_assert(chunk(^^tag<71>) == 100);
static_assert(chunk(^^tag<72>) == 100);
static_assert(chunk(^^tag<73>) == 100);
static_assert(chunk(^^tag<74>) == 100);
static_assert(chunk(^^tag<75>) == 100);
static_assert(chunk(^^tag<76>) == 100);
static_assert(chunk(^^tag<77>) == 100);
static_assert(chunk(^^tag<78>) == 100);
static_assert(chunk(^^tag<79>) == 100);
static_assert(chunk(^^tag<80>) == 100);
static_assert(chunk(^^tag<81>) == 100);
static_assert(chunk(^^tag<82>) == 100);
static_assert(chunk(^^tag<83>) == 100);
static_assert(chunk(^^tag<84>) == 100);
static_assert(chunk(^^tag<85>) == 100);
static_assert(chunk(^^tag<86>) == 100);
static_assert(chunk(^^tag<87>) == 100);
static_assert(chunk(^^tag<88>) == 100);
static_assert(chunk(^^tag<89>) == 100);
static_assert(chunk(^^tag<90>) == 100);
static_assert(chunk(^^tag<91>) == 100);
static_assert(chunk(^^tag<92>) == 100);
static_assert(chunk(^^tag<93>) == 100);
static_assert(chunk(^^tag<94>) == 100);
static_assert(chunk(^^tag<95>) == 100);
static_assert(chunk(^^tag<96>) == 100);
static_assert(chunk(^^tag<97>) == 100);
static_assert(chunk(^^tag<98>) == 100);
static_assert(chunk(^^tag<99>) == 100);
int main() {}
