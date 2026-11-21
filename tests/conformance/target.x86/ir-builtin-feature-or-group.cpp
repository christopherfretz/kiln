// [x86] A builtin's required-feature expression is clang's
// (`TargetFeatures::hasRequiredFeatures`, WP-1841): `|` binds loosest,
// then `,`. `__builtin_ia32_vpmadd52huq128` needs
// `avx512ifma,avx512vl|avxifma`, i.e. both AVX-512 features or AVX-IFMA
// alone; kiln read it as `avx512ifma` and one of the others, and rejected
// a caller with only `avxifma`. (The missing-feature error is a codegen
// error: crates/kiln_driver/tests/codegen.rs
// `builtin_feature_expressions_match_clang`.)
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
typedef long long v2di __attribute__((vector_size(16)));
__attribute__((target("avxifma"))) v2di ifma(v2di a, v2di b, v2di c) {
  return __builtin_ia32_vpmadd52huq128(a, b, c);
}
__attribute__((target("avx512ifma,avx512vl"))) v2di avx512(v2di a, v2di b, v2di c) {
  return __builtin_ia32_vpmadd52huq128(a, b, c);
}
