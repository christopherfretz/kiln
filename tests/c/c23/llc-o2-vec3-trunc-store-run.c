// C23 6.5.17.1, 7.26.2.1 (memcpy); clang's __builtin_convertvector and
// ext_vector_type: converting a 3-lane vector of 64- or 128-bit integers
// to 3 lanes of signed char and copying its 3 bytes stores 1 2 3. At -O1
// and above the optimizer turns the copy into `trunc <3 x iN> to <3 x i8>`
// and `store <3 x i8>`, which AArch64 SelectionDAG lowers wrongly: lane 2
// is written into byte 1 and byte 2 is never written (LLVM 19 and 22;
// GlobalISel at -O0 is right; reported by the hod project,
// github.com/christopherfretz/hod). The IR is clang's, and clang's program
// is wrong the same way, so kiln does not work around it
// (docs/KNOWN_ISSUES.md "LLVM code generator bugs").
//@ expect: run
//@ std: c23
//@ args: -O2
//@ requires: target-aarch64
//@ stdout: 1 2 3 4 5 6
//@ xfail: LLVM AArch64 SelectionDAG stores trunc <3 x iN> to <3 x i8> into the wrong bytes (llc 19 and 22, as clang)
int printf(const char *, ...);
typedef long long l3 __attribute__((ext_vector_type(3)));
typedef signed char c3 __attribute__((ext_vector_type(3)));
typedef __int128 q3 __attribute__((ext_vector_type(3)));
__attribute__((noinline)) void d(void *p, l3 v) {
  c3 t = __builtin_convertvector(v, c3);
  __builtin_memcpy(p, &t, 3);
}
__attribute__((noinline)) void dq(void *p, q3 v) {
  c3 t = __builtin_convertvector(v, c3);
  __builtin_memcpy(p, &t, 3);
}
int main(void) {
  signed char b[4] = {9, 9, 9, 9}, e[4] = {9, 9, 9, 9};
  l3 v = {1, 2, 3};
  q3 w = {4, 5, 6};
  d(b, v);
  dq(e, w);
  printf("%d %d %d %d %d %d\n", b[0], b[1], b[2], e[0], e[1], e[2]);
  return 0;
}
