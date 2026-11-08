// [cpp.predefined] [driver] `-msign-return-address=all` after
// `-mbranch-protection=standard` replaces it: `__ARM_FEATURE_PAC_DEFAULT`
// is 5 (A key, every function) and neither `__ARM_FEATURE_BTI_DEFAULT` nor
// `__ARM_FEATURE_GCS_DEFAULT` is defined, as in clang 19 (spec/pp.md
// "Predefined macros"). WP-846.
//@ requires: target-aarch64
//@ strict-diags
//@ args: -mbranch-protection=standard -msign-return-address=all
#if __ARM_FEATURE_PAC_DEFAULT != 5
#error PAC_DEFAULT
#endif
#if defined(__ARM_FEATURE_BTI_DEFAULT) || defined(__ARM_FEATURE_GCS_DEFAULT)
#error BTI/GCS_DEFAULT
#endif
int main() { return 0; }
