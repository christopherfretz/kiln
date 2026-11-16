// The programs temp-copy-second-step-cxx14.cpp rejects, valid from C++17
// ([dcl.init.general]/16.6.3: the converted prvalue initializes the object
// directly; no copy). Found running GCC's C++ testsuite (WP-1513).
//@ args: -std=c++17
struct R { R(int); };
struct B { B(int); B(B&); B(R); };
B b = 1;
void h(B);
void t1() { h(1); }
B t2() { return 1; }
struct D { operator B(); };
void t3(D d) { h(d); B x = d; (void)x; }

template <class Y> struct Ref { Ref(Y*); };
template <class X> struct P {
  P(P&);
  P(Ref<X>);
  template <class Y> operator Ref<Y>();
  template <class Y> operator P<Y>();
};
struct Base {};
struct Der : Base {};
P<Der> f();
void hb(P<Base>);
void t4() { hb(f()); }
