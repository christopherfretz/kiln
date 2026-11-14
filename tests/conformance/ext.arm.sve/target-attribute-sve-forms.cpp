// [ACLE] The SVE type check consults the target features of the function
// a use is in (clang checkTypeSupport, getFunctionFeatureMap): the
// translation unit's plus the function's target attribute, written on it
// or on an earlier declaration, or pushed by '#pragma clang attribute'
// (also onto member functions, hidden friends of class templates and
// their instantiations, and lambdas' call operators). '+sve', 'sve2',
// 'arch=armv9-a' and an SVE CPU all imply sve. An instantiation of a
// function template with the attribute keeps it. Found building
// open-source SIMD libraries. spec/sema.md "SVE".
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags

__attribute__((target("sve"))) __SVBool_t all(__SVBool_t p) { __SVBool_t q = p; return q; }

__SVInt8_t decl_first(__SVInt8_t);
__attribute__((target("sve"))) __SVInt8_t decl_first(__SVInt8_t a) { __SVInt8_t b = a; return b; }

__attribute__((target("sve"))) __SVInt8_t attr_first(__SVInt8_t);
__SVInt8_t attr_first(__SVInt8_t a) { __SVInt8_t b = a; return b; }

template <class T> __attribute__((target("sve"))) T tmpl(T a) { T b = a; return b; }
template __SVInt8_t tmpl(__SVInt8_t);

__attribute__((target("sve2"))) __SVInt8_t sve2(__SVInt8_t a) { return a; }
__attribute__((target("+sve"))) __SVInt8_t plus_sve(__SVInt8_t a) { return a; }
__attribute__((target("arch=armv9-a"))) __SVInt8_t v9(__SVInt8_t a) { return a; }
__attribute__((target("cpu=a64fx"))) __SVInt8_t cpu(__SVInt8_t a) { return a; }

#pragma clang attribute push(__attribute__((target("sve"))), apply_to = function)
__SVBool_t pushed(__SVBool_t a) { return a; }
void with_lambda() {
  __SVInt8_t v;
  auto l = [&]() { __SVInt8_t y = v; (void)y; };
  (void)l;
}
struct S {
  __SVInt8_t mem(__SVInt8_t a) { return a; }
  friend __SVInt8_t fr(S, __SVInt8_t a) { return a; }
};
template <class T> struct TS {
  friend T hf(TS, T a) { return a; }
};
__SVInt8_t use(TS<__SVInt8_t> t, __SVInt8_t a) { return hf(t, a); }
#pragma clang attribute pop
