// [expr.static.cast]/4, [expr.const.cast]/4, [expr.cast]/4: a cast to a
// reference type of a glvalue of the same type is a glvalue designating the
// same object; used as a value, it is loaded ([conv.lval]) and then
// converted. kiln's codegen took the operand's scalar value, which for a
// call returning a reference is the referee's address, so the following
// integral or floating conversion was of a pointer ("invalid fptoui from
// ptr to i32", "invalid fpext from ptr to double"). The shape of
// boost/optional/detail/optional_trivially_copyable_base.hpp's
// `assign(optional<U>&&)`: `m_storage = static_cast<ref_type>(rhs.get());`
// with `ref_type` = `U&&`. Also: static_cast, const_cast and C-style
// reference casts of function, member function and overloaded operator
// results, of members and array elements, in arithmetic, as call
// arguments, in a conditional, and of a vector.
//@ expect: run
//@ stdout: 7 2.5
//@ stdout: 7 1.5
//@ stdout: 21 21 40
//@ stdout: 20 7
//@ stdout: 3 4 4
//@ stdout: 13
//@ stdout: 20
//@ stdout: 3 4
extern "C" int printf(const char *, ...);
template <class T> struct opt {
  T s;
  bool init;
  template <class U> void assign(opt<U> &&rhs) {
    if (rhs.init) s = static_cast<U &&>(rhs.get());
    init = rhs.init;
  }
  T &get() { return s; }
};
unsigned short us = 7;
unsigned short &getus() { return us; }
float fl = 1.5f;
float &getf() { return fl; }
int gi = 20;
int &geti() { return gi; }
const int &cgeti() { return gi; }
long widen(long x) { return x + 1; }
double dbl(double x) { return x; }
struct W {
  int v[2];
  int &operator[](int i) { return v[i]; }
  int &first() { return v[0]; }
};
typedef int v4 __attribute__((vector_size(16)));
v4 gv = {1, 2, 3, 4};
v4 &getv() { return gv; }
int main() {
  opt<unsigned> a{0, false};
  opt<unsigned short> b{7, true};
  a.assign(static_cast<opt<unsigned short> &&>(b));
  opt<double> c{0, false};
  opt<float> d{2.5f, true};
  c.assign(static_cast<opt<float> &&>(d));
  printf("%u %g\n", a.s, c.s);
  unsigned u = static_cast<unsigned short &&>(getus());
  double x = static_cast<float &&>(getf());
  printf("%u %g\n", u, x);
  printf("%d %ld %d\n", static_cast<int &>(geti()) + 1, widen(const_cast<int &>(cgeti())),
         (int &)geti() * 2);
  printf("%g %g\n", dbl(static_cast<const int &>(geti())), dbl((short &&)getus()));
  W w{{3, 4}};
  long l = static_cast<int &&>(w.first());
  printf("%ld %ld %g\n", l, (long)static_cast<int &>(w[1]), dbl(const_cast<const int &>(w[1])));
  unsigned short arr[2] = {5, 6};
  unsigned sum = static_cast<unsigned short &>(arr[1]) + static_cast<unsigned short &&>(b.s);
  printf("%u\n", sum);
  printf("%d\n", u > 1 ? static_cast<int &>(geti()) : static_cast<int &&>(w.first()));
  v4 vv = static_cast<v4 &&>(getv());
  printf("%d %d\n", vv[2], static_cast<v4 &>(getv())[3]);
  return 0;
}
