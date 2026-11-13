// [expr.reinterpret.cast]/11: `reinterpret_cast<T&>(x)` is an lvalue that
// refers to x's object with type T; its lvalue-to-rvalue conversion
// ([conv.lval]/3) reads a T from that object, i.e. reinterprets the bytes
// through memory. kiln's codegen took the value of the operand instead (a
// `u32` where a `float` was wanted: "call argument 0: expected float,
// found i32"). The shape of a downstream source file's
// `s << reinterpret_cast<const float&>(f.val_)` in a friend of a class
// template. Also: of a reference parameter, an array element, a function's
// and an overloaded operator's reference result, an xvalue, in arithmetic,
// converted to int and double, in a conditional, and the reverse cast.
// `(const float&)val_` is not a reinterpret_cast ([expr.cast]/4: the
// static_cast `const float& t(val_);` comes first) and converts the value.
//@ expect: run
//@ stdout: 3 3
//@ stdout: 5 1.07584e+09
//@ stdout: 2.5 2.5
//@ stdout: 2.5 5
//@ stdout: 2.5 1.5
//@ stdout: 2 2.5 40200000
//@ stdout: 2.5
extern "C" int printf(const char *, ...);
int show(float f) { return f == 2.5f ? 3 : 1; }
double twice(double d) { return d * 2; }
template <int W> struct OF {
  unsigned val_;
  friend int out(const OF &f) { return show(reinterpret_cast<const float &>(f.val_)); }
  float cstyle() const { return (const float &)val_; }
};
int plain(const unsigned &u) { return show(reinterpret_cast<const float &>(u)); }
unsigned g = 0x40200000u;
unsigned &getu() { return g; }
template <class T> T &&xv(T &t) { return static_cast<T &&>(t); }
struct V {
  unsigned a[2];
  unsigned &operator[](int i) { return a[i]; }
};
int main() {
  OF<1> a{0x40200000u};
  unsigned arr[3] = {0, 0x40200000u, 0x3fc00000u};
  V v{{0x3fc00000u, 0x40200000u}};
  float f = 2.5f;
  printf("%d %d\n", out(a), plain(0x40200000u));
  printf("%g %g\n", reinterpret_cast<const float &>(a.val_) * 2.0f, a.cstyle());
  printf("%g %g\n", reinterpret_cast<float &>(arr[1]), (float &)arr[2] + 1);
  printf("%g %g\n", reinterpret_cast<float &>(getu()), twice(reinterpret_cast<float &>(getu())));
  printf("%g %g\n", reinterpret_cast<float &>(v[1]), reinterpret_cast<float &&>(xv(arr[2])));
  int i = reinterpret_cast<float &>(g);
  double d = reinterpret_cast<const float &>(g);
  printf("%d %g %x\n", i, d, reinterpret_cast<unsigned &>(f));
  printf("%g\n", i > 1 ? reinterpret_cast<float &>(arr[1]) : reinterpret_cast<float &>(arr[2]));
  return 0;
}
