// [ACLE] A function whose `target("sve")` attribute enables the `sve`
// feature may declare SVE locals and pointers to them even when the
// translation unit's target lacks it: clang `checkTypeSupport` consults the
// enclosing function's feature map (`getFunctionFeatureMap`), the TU's
// features plus its `target` attribute. spec/types.md §5.1.
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu
//@ xfail(kiln): WP-910 follow-up, the SVE type check ignores the function's target("sve") attribute (sema has only the TU-wide sve flag)

__attribute__((target("sve"))) int f() {
  __SVInt32_t v;
  __clang_svint8x2_t t;
  __SVBool_t *p = nullptr;
  (void)v;
  (void)t;
  return p == nullptr;
}
