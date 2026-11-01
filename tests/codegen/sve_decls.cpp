// The AArch64 SVE builtins mangle as vendor extended types
// (`u <source-name>`), which are substitution candidates. Without the sve
// target feature only pointers to them can appear in a definition.
#ifdef __aarch64__
template <class T> void tf(T *) {}
void g(__SVFloat32_t *, __SVFloat32_t *, __SVBool_t **) {
  tf<__SVInt8_t>(nullptr);
  tf<__SVCount_t>(nullptr);
  tf<__SVBfloat16_t>(nullptr);
}
void h(__SVUint16_t *, __SVInt16_t *, __SVUint64_t *, __SVFloat16_t *,
       __SVFloat64_t *, __SVInt32_t *, __SVInt64_t *, __SVUint8_t *,
       __SVUint32_t *) {}
#endif
