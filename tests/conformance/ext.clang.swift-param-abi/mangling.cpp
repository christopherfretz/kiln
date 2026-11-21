// Itanium mangling of the Swift parameter ABIs (clang
// `CXXNameMangler::mangleExtParameterInfo`): inside a function type each
// non-ordinary parameter is prefixed by its vendor qualifier
// (`U13swift_context`, `U19swift_async_context`, `U18swift_error_result`,
// `U21swift_indirect_result`), which is not a substitution candidate; a
// function's own name mangles its parameter types only. spec/abi.md
// §8.33, WP-1745.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define SC __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
#define CTX __attribute__((swift_context))
#define ACTX __attribute__((swift_async_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
SC void f1(IND void *out, int x, CTX void *ctx, ERR void **err) {}
SA void f2(ACTX void *a) {}
void f3(ACTX void *a) {}
SC void g(void (*fp)(CTX void *, ERR void **) SC) {}
SC void h(void (*fp)(void *, void **) SC) {}
template <class T> void tf(T) {}
template <class... T> using FP = SC void (*)(IND T..., CTX void *);
void inst() {
  tf(&f1);
  tf(&f2);
  tf(&f3);
  tf(FP<int *, long *>());
  tf(FP<>());
}
