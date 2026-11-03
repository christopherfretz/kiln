// WP-492: the return-adjusting thunk of a variadic virtual function is a
// clone of its method's body (clang `GenerateVarArgsThunk`): appended to
// the module with the method's attributes, `this` adjusted before its
// first store in the entry block, the value of the first `ret` adjusted
// (null-checked for a pointer, the blocks placed after the returning
// block; a constant folded) and returned. Covers a non-virtual and a
// virtual-base return adjustment, a this adjustment, a reference, an
// inline (`linkonce_odr`) method with cleanups and a landing pad, and a
// constant return. [class.virtual]/8.
struct Base { virtual Base *f(int, ...); virtual Base &r(int, ...); };
struct Other { virtual void o(); long z; };
struct Ret : Other, Base {};
struct VRet : virtual Base { long q; };
Ret gr;
struct D : Base {
  Ret *f(int, ...) override;
  Ret &r(int, ...) override;
};
Ret *D::f(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  int v = __builtin_va_arg(ap, int);
  __builtin_va_end(ap);
  if (v)
    return &gr;
  return nullptr;
}
Ret &D::r(int, ...) { return gr; }
struct A { virtual A *g(int, ...); };
struct B : Other, A { B *g(int, ...) override; };
B *B::g(int n, ...) { return n ? this : nullptr; }
struct E : Base { VRet *f(int, ...) override; };
VRet *E::f(int, ...) { return nullptr; }
struct G : Base { Ret *f(int, ...) override; };
Ret *G::f(int, ...) { return &gr; }
struct Dtor { ~Dtor(); };
void may_throw();
struct F : Other, A {
  F *g(int n, ...) override {
    Dtor d;
    try { may_throw(); } catch (...) { return nullptr; }
    return this;
  }
};
F gf;
