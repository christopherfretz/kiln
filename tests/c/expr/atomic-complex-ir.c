// An `_Atomic _Complex` object (C17 6.2.5/20, 6.5.16.2/3) is loaded and
// stored whole, atomically and sequentially consistent, through a
// temporary of the atomic type (an atomic integer access when the size
// is lock-free, else the `__atomic_load`/`__atomic_store` library
// calls), and its parts are read from or written to that temporary; the
// `__c11_atomic_*` builtins on one return the complex value (WP-1739,
// clang `EmitAtomicLoad`/`EmitAtomicStore` of a complex).
//@ expect: ir
_Atomic _Complex double ad;
_Atomic _Complex float af;
_Complex double xd;
_Complex float xf;
void use(_Complex float);
void f1(void) { _Atomic _Complex double y = xd * 2.0; }
void f2(void) { __c11_atomic_init(&af, xf + 1.0f); }
void f3(void) { __c11_atomic_store(&af, xf * xf, 5); }
_Complex float f4(void) { return __c11_atomic_load(&af, 5); }
_Complex double f5(void) { return __c11_atomic_exchange(&ad, xd, 2); }
_Bool f6(_Complex float *e) { return __c11_atomic_compare_exchange_strong(&af, e, xf, 5, 5); }
_Complex float g1(void) { return af; }
void g2(_Complex float v) { af = v; }
void g3(void) { af += 1.0f; }
void g4(void) { af++; --af; }
_Complex float g5(void) { return (af = xf); }
int g6(void) { return af == xf; }
_Bool g7(void) { return af; }
void g8(void) { use(af); }
_Complex double g9(void) { ad *= xd; return ad; }
float g10(void) { return __real__ (_Complex float)af; }
void g11(volatile _Atomic _Complex float *p) { *p = xf; xf = *p; }
