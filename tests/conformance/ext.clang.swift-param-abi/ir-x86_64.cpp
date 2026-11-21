// clang `ParameterABIAttr` lowering on x86_64 (clang `ConstructAttributeList`,
// `EmitFunctionProlog`, `EmitCall`): `swift_context` is `swiftself`,
// `swift_async_context` `swiftasync` (also under the default
// convention), `swift_error_result` `swifterror`, `swift_indirect_result`
// `noalias` plus `dereferenceable`/`align` of a complete pointee and
// `sret` on the first one of a `void` function without another `sret`.
// A `swift_error_result` argument is passed as an `alloca swifterror`
// temporary written back after the call; the callee works on a
// `swifterror.temp` copied back at its end. Pack expansions and class
// template members carry the ABIs. spec/abi.md §8.33, WP-1745.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define SC __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
#define CTX __attribute__((swift_context))
#define ACTX __attribute__((swift_async_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
struct Big { long a, b, c, d, e; };
struct S { int a; double b; };
struct Inc;
SC void f1(IND void *out, int x, CTX void *ctx, ERR void **err);
SC void f1(IND void *out, int x, CTX void *ctx, ERR void **err) { *err = ctx; }
SA void f2(ACTX void *a) {}
void f3(ACTX void *a) {}
SC void call(void *c) {
  void *e = nullptr;
  f1(nullptr, 1, c, &e);
}
SC void callp(void (*fp)(CTX void *, ERR void **) SC, void *c) {
  void *e = c;
  fp(c, &e);
}
SC void cref(CTX void *c, ERR void *&e) { e = c; }
SC int ret_err(CTX void *c, ERR void **e) {
  if (c)
    return 3;
  *e = c;
  return 4;
}
SC Big bigret(IND void *o, CTX void *c) { return {}; }
SC void i1(IND S *o, IND int &r, IND Inc *q, int x) {}
SC int i2(IND S *o) { return 0; }
template <class... T> SC void pk(IND T... o, CTX void *c, ERR void **e) {}
void usepk() {
  void *e = nullptr;
  pk<int *, S *>(nullptr, nullptr, nullptr, &e);
}
template <class T> struct W {
  SC void m(CTX T c, ERR void **e) {}
};
void usew() {
  void *e = nullptr;
  W<void *>().m(nullptr, &e);
}
