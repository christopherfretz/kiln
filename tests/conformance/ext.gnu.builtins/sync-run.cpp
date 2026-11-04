//@ expect: run
//@ diag: 30:39 warning "the semantics of this intrinsic changed with GCC version 4.4 - the newer semantics are provided here"
//@ diag: 39:39 warning "the semantics of this intrinsic changed with GCC version 4.4 - the newer semantics are provided here"
//@ stdout: fetch 0 5 2 7 4 3 -2
//@ stdout: post 5 3 7 3 0 -1
//@ stdout: cas 0 3 1 0 4
//@ stdout: lock 4 9 0 0 1
//@ stdout: sized 3 300 70000 5000000000
//@ stdout: ptr 1 2 1 1
//@ stdout: misc 1 7 3 9 1
// GCC "Legacy __sync Built-in Functions for Atomic Memory Access": each
// operation is overloaded on the pointee, an integer or pointer of 1, 2,
// 4, 8 or 16 bytes (clang `Sema::BuiltinAtomicOverloaded` retargets the
// call to `__sync_<op>_<N>`); `fetch_and_<op>` returns the old value,
// `<op>_and_fetch` the new one (nand is `~(old & v)`, the GCC 4.4
// semantics), the compare-and-swaps the old value or whether it matched,
// `lock_test_and_set` the old value, and `lock_release` stores 0. BDE's
// bsls_atomicoperations_x64_all_gcc.h and boost's sp_counted_base_sync.hpp use
// these.
extern "C" int printf(const char *, ...);

int main() {
  int i = 0;
  int f0 = __sync_fetch_and_add(&i, 5);
  int f1 = __sync_fetch_and_sub(&i, 3);
  int f2 = __sync_fetch_and_or(&i, 5);
  int f3 = __sync_fetch_and_and(&i, 4);
  int f4 = __sync_fetch_and_xor(&i, 5);
  i = 3;
  int f5 = __sync_fetch_and_nand(&i, 1); // i = ~(3 & 1) = -2
  printf("fetch %d %d %d %d %d %d %d\n", f0, f1, f2, f3, f4, f5, i);

  int j = 0;
  int p0 = __sync_add_and_fetch(&j, 5);
  int p1 = __sync_sub_and_fetch(&j, 2);
  int p2 = __sync_or_and_fetch(&j, 4);
  int p3 = __sync_and_and_fetch(&j, 3);
  int p4 = __sync_xor_and_fetch(&j, 3);
  int p5 = __sync_nand_and_fetch(&j, 0);
  printf("post %d %d %d %d %d %d\n", p0, p1, p2, p3, p4, p5);

  long x = 0;
  long c0 = __sync_val_compare_and_swap(&x, 0, 3);
  long c1 = __sync_val_compare_and_swap(&x, 7, 9);
  bool c2 = __sync_bool_compare_and_swap(&x, 3, 4);
  bool c3 = __sync_bool_compare_and_swap(&x, 3, 5);
  printf("cas %ld %ld %d %d %ld\n", c0, c1, c2, c3, x);

  unsigned u = 4;
  unsigned l0 = __sync_lock_test_and_set(&u, 9);
  unsigned l1 = u;
  __sync_lock_release(&u);
  __sync_synchronize();
  bool flag = false;
  bool l2 = __sync_lock_test_and_set(&flag, true);
  printf("lock %u %u %u %d %d\n", l0, l1, u, l2, flag);

  signed char sc = 1;
  short s = 100;
  int w = 69999;
  long long ll = 4999999999;
  __sync_add_and_fetch(&sc, 2);
  __sync_fetch_and_add_2(&s, 200);
  __sync_add_and_fetch(&w, 1);
  __sync_add_and_fetch(&ll, 1);
  printf("sized %d %d %d %lld\n", sc, s, w, ll);

  int arr[4] = {};
  int *ptr = arr;
  int *o0 = __sync_val_compare_and_swap(&ptr, arr + 1, arr + 3); // no swap
  int *o1 = __sync_val_compare_and_swap(&ptr, arr, arr + 2);
  int *o2 = __sync_lock_test_and_set(&ptr, arr + 1);
  printf("ptr %d %d %d %d\n", o0 == arr, int(o2 - o1),
         ptr == arr + 1, o1 == arr);

  enum E { A = 1, B = 6 } e = A;
  int m0 = __sync_fetch_and_add(&e, B);       // the value is an E
  short sh = 3;
  short m1 = __sync_fetch_and_add(&sh, (long)3); // converted to short
  printf("misc %d %d %d %d %d\n", m0, (int)e, m1, sh + 3, (int)__sync_fetch_and_or(&sh, 0) == 6);
}
