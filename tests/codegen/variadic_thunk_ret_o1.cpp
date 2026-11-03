// WP-492: `variadic_thunk_ret.cpp` above -O0: the clone keeps the
// method's -O1 body (TBAA, lifetime markers) and attributes; the vtable's
// reference to the thunk, emitted first, is the same function.
// [class.virtual]/8.
//@ args: -O1
struct Base { virtual Base *f(int, ...); };
struct Other { virtual void o(); long z; };
struct Ret : Other, Base {};
Ret gr;
struct D : Other, Base {
  D();
  Ret *f(int, ...) override;
};
D::D() {}
Ret *D::f(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  int v = __builtin_va_arg(ap, int);
  __builtin_va_end(ap);
  return v ? &gr : nullptr;
}
