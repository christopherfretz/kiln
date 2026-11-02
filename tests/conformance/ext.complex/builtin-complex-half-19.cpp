// `__builtin_complex` with `_Float16` or `__fp16` operands is rejected by
// clang 19 at the callee (WP-380; spec/sema.md §5.16); clang 22 accepts
// both (builtin-complex-half.cpp).
//@ expect: compile-fail
//@ requires: clang<22
//@ strict-diags
//@ diag: 10:11 error "'_Complex _Float16' is invalid"
//@ diag: 11:10 error "'_Complex half' is invalid"
//@ diag: 12:30 error "'_Complex _Float16' is invalid"
auto b = (__builtin_complex((_Float16)1, (_Float16)2));
auto h = __builtin_complex((__fp16)1, (__fp16)2);
void g() { (void)(int)sizeof __builtin_complex((_Float16)1, (_Float16)2); }
