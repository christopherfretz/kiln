// Swift parameter ABIs across compilers: the error value of a
// `swift_error_result` parameter travels in the `swifterror` register
// both ways between a kiln-built and a clang-built unit (the caller's
// `alloca swifterror` temporary and its write-back, the callee's copy
// back on each return), `swift_context` in `swiftself` and a
// `swift_indirect_result` in `sret`. spec/abi.md §8.33, WP-1745.
//@ expect: run
//@ clang-unit: Inputs/swift-error-other.cpp
//@ exit: 42
#define SC __attribute__((swiftcall))
#define CTX __attribute__((swift_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
SC int other_set(CTX void *ctx, ERR void **err);
SC int other_calls_test(void *c);
SC void other_indirect(IND int *out, int v);
SC int test_ret(CTX void *ctx, ERR void **err) {
  if (ctx) {
    *err = ctx;
    return 1;
  }
  return 0;
}
int main() {
  int x = 0;
  void *e = nullptr;
  int r = other_set(&x, &e);
  if (r != 7 || e != &x)
    return 1;
  r = other_set(nullptr, &e);
  if (r != -1 || e != &x)
    return 2;
  r = other_calls_test(&x);
  if (r != 10)
    return 3;
  int out = 0;
  other_indirect(&out, 16);
  if (out != 32)
    return 4;
  return 42;
}
