// [ACLE] Fixed-length SVE types at run time (ARM ACLE §3.7.3.3): values
// stored in globals, members and arrays, GNU-vector arithmetic, subscripts,
// conversions to and from the sizeless type across calls, and predicates.
// Needs an SVE CPU (`requires: host-sve`). 128 bits is the one vector
// length every SVE CPU implements, so the program sets it (Linux
// `PR_SVE_SET_VL`) and exits 99 only if that fails; 256-bit layout is
// checked by layout-sve-vector-bits.cpp, which does not execute SVE code.
// spec/landed/codegen/WP-810.md (WP-810).
//@ expect: run
//@ requires: target-aarch64
//@ requires: host-sve
//@ args: -march=armv8-a+sve -msve-vector-bits=128
//@ stdout: 10 4 1 0 1 6 -2
#include <stdio.h>
#include <sys/prctl.h>
typedef __SVInt32_t svint32_t; typedef __SVBool_t svbool_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(128)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(128)));
typedef int gv __attribute__((vector_size(16)));

struct S { fi32 v; fb p; int n; };
fi32 g;
fi32 arr[2];
S gs;

__attribute__((noinline)) svint32_t through(svint32_t s) { return s; }
__attribute__((noinline)) fi32 twice(fi32 a) { return a + a; }
__attribute__((noinline)) fb keep(fb p) { return p; }
__attribute__((noinline)) int run();

int main() {
  // PR_SVE_SET_VL (50): 16-byte vectors for this thread; the result's low
  // 16 bits are the length set.
  if ((prctl(50, 16, 0, 0, 0) & 0xffff) != 16) return 99;
  return run();
}

int run() {
  fi32 a = {1, 2, 3, 4};
  g = through(a);
  arr[1] = twice(g);
  gs.v = arr[1] - a;
  int sum = 0;
  for (int i = 0; i < 4; ++i) sum += gs.v[i];
  gv x = gs.v;
  fb all = keep(fb{0xff, 0xff});
  gs.p = all;
  fi32 c = a > 3;
  printf("%d %d %d %d %d %d %d\n", sum, x[3], gs.p[0] == 0xff, c[2], c[3] == -1 ? 1 : 0,
         twice(a)[2], ~a[0]);
  return 0;
}
