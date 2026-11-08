// [cpp.predefined] [driver] `-mbranch-protection=` defines clang 19's
// `__ARM_FEATURE_PAC_DEFAULT` (bit 0 the A key, bit 1 the B key, bit 2
// every function, bit 3 PAuth-LR), `__ARM_FEATURE_BTI_DEFAULT` and
// `__ARM_FEATURE_GCS_DEFAULT`; `-msign-return-address=` given after it
// replaces it (spec/pp.md "Predefined macros"). WP-846.
//@ requires: target-aarch64
//@ strict-diags
//@ args: -mbranch-protection=pac-ret+leaf+b-key+bti+gcs
#if __ARM_FEATURE_PAC_DEFAULT != 6
#error PAC_DEFAULT
#endif
#if __ARM_FEATURE_BTI_DEFAULT != 1 || __ARM_FEATURE_GCS_DEFAULT != 1
#error BTI/GCS_DEFAULT
#endif
int main() { return 0; }
