// [class.virtual]/8: a covariant override of a variadic virtual function
// called through the base returns a pointer (or reference) converted to
// the base's return type; the thunk is a clone of the overrider's body
// (the variadic arguments cannot be forwarded through a call), with the
// this and return adjustments, null-checked for a pointer.
//@ expect: run
//@ stdout: 6 1 1 9 1 1 7 1
#include <cstdarg>
#include <cstdio>
struct Other { virtual void o() {} long z = 0; };
struct Base { virtual Base *f(int, ...) { return nullptr; } virtual Base &r(int, ...) { return *this; } };
struct Ret : Other, Base { long sum = 0; };
struct VRet : Other, virtual Base { long q = 7; };
Ret gr;
VRet gv;
struct D : Other, Base {
  Ret *f(int n, ...) override {
    va_list ap;
    va_start(ap, n);
    long s = 0;
    for (int i = 0; i < n; ++i) s += va_arg(ap, int);
    va_end(ap);
    gr.sum = s;
    return s ? &gr : nullptr;
  }
  Ret &r(int n, ...) override {
    va_list ap;
    va_start(ap, n);
    gr.sum = va_arg(ap, long);
    va_end(ap);
    return gr;
  }
};
struct E : Base { VRet *f(int, ...) override { return &gv; } };
int main() {
  D d;
  Base *b = &d;
  Base *p = b->f(3, 1, 2, 3);
  std::printf("%ld %d ", gr.sum, p == static_cast<Base *>(&gr));
  std::printf("%d ", b->f(0) == nullptr);
  Base &q = b->r(1, 9L);
  std::printf("%ld %d ", gr.sum, &q == static_cast<Base *>(&gr));
  E e;
  Base *eb = &e;
  Base *v = eb->f(0);
  std::printf("%d %ld %d\n", v == static_cast<Base *>(&gv), gv.q, v != nullptr);
}
