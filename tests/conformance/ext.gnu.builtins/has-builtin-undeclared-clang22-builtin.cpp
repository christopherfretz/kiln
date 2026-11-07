// [cpp.cond]/5, clang `__has_builtin`: a `__builtin_*` function clang 22
// added that kiln does not declare is not advertised, so a use guarded by
// `__has_builtin` (abseil's `__builtin_infer_alloc_token` test) is skipped
// instead of naming an undeclared identifier. Names kiln declares keep
// clang's answer.
//@ expect: compile-ok
#if __has_builtin(__builtin_infer_alloc_token)
unsigned long tok = __builtin_infer_alloc_token(sizeof(int));
#endif
#if __has_builtin(__builtin_elementwise_popcount) || __has_constexpr_builtin(__builtin_bswapg)
int p = __builtin_elementwise_popcount(3) + __builtin_bswapg(3);
#endif
#if __has_builtin(__builtin_trivially_relocate)
void r(int* d, int* s) { __builtin_trivially_relocate(d, s, 1); }
#endif
static_assert(__has_builtin(__builtin_expect) && __has_constexpr_builtin(__builtin_popcount), "");
int main() {}
