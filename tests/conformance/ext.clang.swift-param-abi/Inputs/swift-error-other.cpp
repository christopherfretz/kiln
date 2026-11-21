// The clang-built half of swift-error-two-unit-run.cpp.
#define SC __attribute__((swiftcall))
#define CTX __attribute__((swift_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
SC int other_set(CTX void *ctx, ERR void **err) {
  if (*err)
    return -1;
  *err = ctx;
  return 7;
}
SC int test_ret(CTX void *ctx, ERR void **err);
SC int other_calls_test(void *c) {
  void *e = nullptr;
  int r = test_ret(c, &e);
  if (r != 1 || e != c)
    return 0;
  e = nullptr;
  r = test_ret(nullptr, &e);
  return r == 0 && e == nullptr ? 10 : 0;
}
SC void other_indirect(IND int *out, int v) { *out = v * 2; }
