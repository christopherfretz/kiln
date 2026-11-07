// [except.throw]/2, [except.handle]/3 under -fno-rtti (a clang/GCC
// extension; [expr.typeid] and [expr.dynamic.cast] are rejected, see
// expr.typeid/no-rtti.cpp): every vtable's RTTI slot is null, and the
// `type_info` objects exception handling still needs are `linkonce_odr`
// definitions in the TU that throws or catches, never references to the
// key function's TU, which may not define them either (clang
// `shouldEmitRTTI`, `ShouldUseExternalRTTIDescriptor`,
// `getTypeInfoLinkage`; spec/codegen.md "RTTI", under -fno-rtti).
// Otherwise a kiln object fails to link against a clang -fno-rtti
// library (`undefined reference to typeinfo for ...`). Compared with
// clang 22's IR exactly; the -19 sibling pins clang 19's global order.
// WP-720.
//@ expect: ir
//@ requires: clang>=22
//@ args: -fno-rtti -ftrivial-auto-var-init=uninitialized
struct Extern {           // key function defined in another TU
  virtual ~Extern();
  virtual int f() const;
};
struct Inline : Extern {  // no key function: linkonce_odr vtable
  int f() const override { return 2; }
};
struct Keyed : Extern {   // key function defined here: strong vtable
  int f() const override;
};
int Keyed::f() const { return 3; }

[[noreturn]] void thrower() { throw Inline(); }

int catcher(int x) {
  int r = 0;
  try {
    if (x) thrower();
  } catch (const Extern& e) {
    r = e.f();
  } catch (int*) {
    r = 1;
  }
  return r;
}

int use(const Extern& e) { return e.f(); }
int make() { Inline i; Keyed k; return use(i) + use(k); }
