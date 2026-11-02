// Mangling differential: a function parameter named in a `decltype` of a
// function template's signature (Itanium ABI §5.1.6 <function-param>:
// `fp <cv> _` from the result type, `fL <L-1> p ... _` from a parameter
// type or a nested function type; clang `mangleFunctionParam`). WP-298:
// libc++'s `-> decltype(ctx.out())` used to mangle as `u7unknown`.
struct Ctx { int* out() { return nullptr; } };
struct S {
  template <class C> auto f(C& ctx) const -> decltype(ctx.out()) { return ctx.out(); }
};
template <class T> auto add(T x, T y) -> decltype(x + y) { return x + y; }
template <class T> auto ident(const T x) -> decltype(x) { return x; }
template <class T> void same(T x, decltype(x) y) {}
template <class T> auto fnptr(T x) -> void (*)(decltype(x)) { return nullptr; }
template <class T> void cb(T x, void (*)(decltype(x))) {}
template <class T, class U> auto second(T, volatile U u) -> decltype(u) { return u; }
void q(int) {}
int use() {
  Ctx c;
  S s;
  s.f(c);
  same(1, 2);
  fnptr(1);
  cb(1, q);
  ident(1);
  second(1, 2L);
  return add(1, 2);
}
