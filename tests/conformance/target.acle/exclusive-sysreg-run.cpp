// [driver] The exclusive accesses and the system-register reads agree with
// their definitions at -O2 (at -O0 the spills between the exclusive load
// and store clear the reservation on some cores and the loop never ends,
// under clang too): an `__builtin_arm_ldrex`/`strex` retry loop
// (and the acquire/release `ldaex`/`stlex` one) increments 8-, 32-, 64- and
// 128-bit, `double` and pointer objects, `clrex` drops a reservation, and
// the virtual counter `__builtin_arm_rsr64("cntvct_el0")` (and its
// upper-case spelling) never goes backwards
// (spec/codegen.md "Target builtins"). WP-846.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O2
//@ strict-diags
//@ stdout: ok 9
extern "C" int printf(const char *, ...);
#include <stdint.h>

static int fails;
static int checks;
static void check(bool c, const char *what) {
  ++checks;
  if (!c) {
    ++fails;
    printf("FAIL %s\n", what);
  }
}

template <class T, class U> static T bump(T *p, U by) {
  T old;
  do {
    old = __builtin_arm_ldrex(p);
  } while (__builtin_arm_strex(old + by, p));
  return old;
}

template <class T, class U> static T bump_acq_rel(T *p, U by) {
  T old;
  do {
    old = __builtin_arm_ldaex(p);
  } while (__builtin_arm_stlex(old + by, p));
  return old;
}

int main() {
  unsigned char c = 250;
  for (int i = 0; i < 10; ++i)
    bump(&c, 1);
  check(c == 4, "8-bit wraps");
  int x = 0;
  for (int i = 0; i < 100; ++i)
    bump_acq_rel(&x, 3);
  check(x == 300, "32-bit acq/rel");
  int64_t y = INT64_C(1) << 40;
  check(bump(&y, -1) == INT64_C(1) << 40 && y == (INT64_C(1) << 40) - 1, "64-bit");
  __int128 z = (__int128)1 << 100;
  bump_acq_rel(&z, 7);
  check(z == ((__int128)1 << 100) + 7, "128-bit");
  double d = 1.5;
  bump(&d, 0.25);
  check(d == 1.75, "double");
  int arr[4] = {};
  int *p = arr;
  bump(&p, 3);
  check(p == arr + 3, "pointer");
  (void)__builtin_arm_ldrex(&x);
  __builtin_arm_clrex();
  check(x == 300, "clrex");

  uint64_t prev = __builtin_arm_rsr64("cntvct_el0");
  bool mono = true;
  for (int i = 0; i < 1000; ++i) {
    uint64_t now = __builtin_arm_rsr64("cntvct_el0");
    mono = mono && now >= prev;
    prev = now;
  }
  check(mono, "cntvct_el0 monotonic");
  uint64_t a = __builtin_arm_rsr64("cntvct_el0");
  uint64_t b = __builtin_arm_rsr64("CNTVCT_EL0");
  check(b >= a, "upper-case spelling");
  if (fails == 0)
    printf("ok %d\n", checks);
  return fails != 0;
}
