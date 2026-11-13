// [expr.reinterpret.cast]/11: a glvalue of type T1 can be cast to "reference
// to T2" if a "pointer to T1" can be reinterpret_cast to "pointer to T2"
// (/7, /2: not casting away constness), the result referring to the same
// object. So T2 may be T1 itself ([expr.reinterpret.cast]/2), T1 with more
// cv-qualifiers, another object type, a function type for a function, and
// the reference an rvalue reference, or an lvalue reference to an xvalue
// (clang accepts any glvalue). The shape of a downstream source file:
// `template<class U> F(const U& u) : v(reinterpret_cast<const T&>(u))` with
// T = U.
//@ expect: run
//@ exit: 36
struct S { int a; };
int f() { return 7; }
template <class T> struct F {
  T v;
  template <class U> F(const U &u) : v(reinterpret_cast<const T &>(u)) {}
};
template <class T> T &&xv(T &t) { return static_cast<T &&>(t); }
int main() {
  const float x = 2.5f;
  const float &r1 = reinterpret_cast<const float &>(x);
  float y = 1.5f;
  float &r2 = reinterpret_cast<float &>(y);
  const float &r3 = reinterpret_cast<const float &>(y);
  const volatile float &r4 = reinterpret_cast<const volatile float &>(y);
  unsigned &r5 = reinterpret_cast<unsigned &>(y);
  S s{3};
  S &r6 = reinterpret_cast<S &>(s);
  int (&r7)() = reinterpret_cast<int (&)()>(f);
  int (&r8)(int) = reinterpret_cast<int (&)(int)>(f);
  float &&r9 = reinterpret_cast<float &&>(y);
  float &r10 = reinterpret_cast<float &>(xv(y));
  float &&r11 = reinterpret_cast<float &&>(xv(y));
  const float &&r12 = reinterpret_cast<const float &&>(x);
  int *p = nullptr;
  int *&r13 = reinterpret_cast<int *&>(p);
  const int *const &r14 = reinterpret_cast<const int *const &>(p);
  F<float> fo(y);
  F<unsigned> fu(y);
  return (int)r1 + (int)r2 + (int)r3 + (int)r4 + (&r5 == (void *)&y) + r6.a + r7() +
         ((void *)&r8 == (void *)&f) + (int)r9 + (int)r10 + (int)r11 + (int)r12 +
         (&r13 == &p) + (r14 == p) + (int)fo.v + (fu.v == r5) + 10;
}
