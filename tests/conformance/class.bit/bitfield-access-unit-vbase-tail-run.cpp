// [class.bit]/1, [class.mi]/4, [intro.object]/9 ([[no_unique_address]]): the
// trailing bit-field `a` sits at bytes 8-10 and the virtual base `Char` is
// placed in the tail padding at byte 11, below the non-virtual size 12.
// The bit-field's access unit must stop at the virtual base (clang
// `calculateTailClippingOffset`); kiln widened it to 4 bytes over `Char`
// and crashed laying out the IR struct (WP-971), and a release build
// clobbered `Char::c` with every store to `a`. The twin `N` has the same
// members with a non-virtual base, which is laid out first.
//@ expect: run
//@ strict-diags
//@ stdout: D x ffffff 16
//@ stdout: N y ffffff
#include <cstdio>

struct __attribute__((aligned(4))) Empty {};
struct Char {
  char c;
};

struct D : virtual Char {
  [[no_unique_address]] Empty e0;
  [[no_unique_address]] Empty e1;
  unsigned a : 24;
};

struct N : Char {
  [[no_unique_address]] Empty e0;
  [[no_unique_address]] Empty e1;
  unsigned a : 24;
};

int main() {
  D d;
  d.c = 'x';
  d.a = 0xffffff;
  std::printf("D %c %x %d\n", d.c, unsigned(d.a), int(sizeof(D)));
  N n;
  n.c = 'y';
  n.a = 0xffffff;
  std::printf("N %c %x\n", n.c, unsigned(n.a));
}
