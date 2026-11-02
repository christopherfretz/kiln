// [basic.extended.fp] (P1467; WP-361): _Float16 mangles as `DF16_`;
// __bf16 as the vendor type `u6__bf16` on AArch64 (not a substitution
// candidate, unlike the SVE types) and `DF16b` on x86-64.
void a(_Float16, _Float16) {}
void b(__bf16, __bf16, __bf16 *) {}
void c(const __bf16 &, __bf16 *, __bf16 *) {}
template <class T> void t(T, T *) {}
void use() {
  t<__bf16>(0, nullptr);
  t<_Float16>(0, nullptr);
  t<const __bf16 *>(nullptr, nullptr);
}
