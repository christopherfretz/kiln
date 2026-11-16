// C++14 [dcl.init]/17.6.3, [over.best.ics]/4: the accepted side of
// temp-copy-second-step-cxx14.cpp. The copy of the temporary is viable
// through a `const` copy constructor or a move constructor; a
// direct-initialization makes no temporary; and copy-initialization from a
// prvalue of the same class ([dcl.init]/17.6.1, the `auto_ptr` idiom
// `P<Der> x = f();`) may reach `P(Ref<X>)` through a conversion function.
// Found running GCC's C++ testsuite (WP-1513).
//@ args: -std=c++14
struct R { R(int); };
struct B { B(int); B(B&); B(R); };
B direct(1);
const B& bound = 1;
struct C { C(int); C(const C&); };
C c = 1;
void hc(C);
struct M { M(int); M(M&&); };
M m = 1;
struct D { operator C(); };
void t1(D d) { hc(1); hc(d); C x = d; (void)x; }
C t2() { return 1; }

template <class Y> struct Ref { Ref(Y*); };
template <class X> struct P {
  P(P&);
  P(Ref<X>);
  template <class Y> operator Ref<Y>();
};
struct Der {};
P<Der> f();
void g(P<Der>);
void t3() { P<Der> x = f(); g(f()); (void)x; }
