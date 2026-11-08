// [over.over]/1, /3, [temp.inst]/5: `&member<P>::wrap<&C::f>` initializing
// an `int (*)(void*, int)` selects the two-parameter `wrap`; the
// one-parameter `wrap<&C::f>`, which the explicit argument alone completes,
// is not used, so its body (calling the one-parameter `f` with none) is
// never instantiated. Boost.FunctionTypes `interface_example.cpp:16`
// (`interface.hpp:143`) hits this.
//@ expect: run
//@ stdout: 5
extern "C" int printf(const char*, ...);
struct C { int f(int v) const { return v + 1; } };
template<class P> struct member {
  template<P M> static int wrap(void* c) { return (static_cast<C*>(c)->*M)(); }
  template<P M, class T0> static int wrap(void* c, T0 a0) { return (static_cast<C*>(c)->*M)(a0); }
};
int main() {
  int (*fp)(void*, int) = &member<int (C::*)(int) const>::wrap<&C::f>;
  C c;
  printf("%d\n", fp(&c, 4));
}
