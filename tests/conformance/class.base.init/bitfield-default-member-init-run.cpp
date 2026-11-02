// [class.base.init]/9, [class.bit]/1: a constructor initializes a bit-field
// from its default member initializer (or a mem-initializer) without
// touching the neighbouring bits of the storage unit, and every bit-field of
// a later storage unit is initialized too (libc++ `__format_spec::__parser`,
// whose `__width_as_arg_ : 1 {false}` follows `uint8_t __reserved_1_ : 6`;
// WP-286). The stack is dirtied first so a missed store shows. Matches
// clang 19.
//@ expect: run
//@ stdout: 0 0 0 0 0 0 0 -1 | 1 5 1 1 -3 7
#include <cstdio>
#include <cstdint>
struct P {
  uint8_t al : 3 {0};
  uint8_t sg : 2 {0};
  bool alt : 1 {false};
  uint8_t ty{0};
  uint8_t r0 : 2 {0};
  uint8_t r1 : 6 {0};
  bool wa : 1 {false};
  bool pa : 1 {false};
  int32_t width{0};
  int32_t prec{-1};
};
struct F { P p; };
struct G : F {};
struct M {
  bool x : 1;
  uint8_t y : 6;
  bool z : 1 {true};
  int w : 5 {-3};
  uint8_t v : 3;
  M() : x(true), y(5), v(7) {}
};
__attribute__((noinline)) void dirty() {
  volatile unsigned char b[256];
  for (int i = 0; i < 256; ++i) b[i] = 0xff;
}
__attribute__((noinline)) void use() {
  G g;
  std::printf("%d %d %d %d %d %d %d %d | ", g.p.al, g.p.sg, g.p.alt, g.p.r1, g.p.wa, g.p.pa,
              g.p.width, g.p.prec);
}
__attribute__((noinline)) void use2() {
  M m;
  std::printf("%d %d %d %d %d %d\n", m.x, m.y, m.z, m.z && m.x, m.w, m.v);
}
int main() {
  dirty();
  use();
  dirty();
  use2();
}
