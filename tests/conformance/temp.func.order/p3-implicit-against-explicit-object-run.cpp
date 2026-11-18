// [temp.func.order]/3, /6, [temp.deduct.partial]/9, CWG2834 (open): partial
// ordering of an implicit object member function template against an
// explicit object one. The text gives every member function the inserted
// parameter X(M) and so does not say how such a pair is ordered; kiln
// follows CWG2834's direction as clang 22 does: only the implicit object
// member gets X(M), a reference to its class with its cv-qualifiers (an
// rvalue reference for `&&`, or without a ref-qualifier against an rvalue
// reference object parameter), and the explicit object parameter is
// compared as declared. With the object types alike, the constraints decide
// (/6.2.2); the same holds for an implicit object member operator against a
// non-member one. clang 22 and GCC 16.1 pick the same candidate for each
// call below (the rejected siblings:
// `p3-implicit-against-explicit-object-ambiguous.cpp`; a static member:
// `p3-static-against-explicit-object-run.cpp`; docs/status/deviations.md).
// Found running GCC's C++ testsuite (WP-1670).
//@ std: c++23
//@ expect: run
//@ stdout: 12 12 12 12 12 12 12 12 12 12 12 12 14 14 14 14 14
extern "C" int printf(const char *, ...);
template <class T> concept C = true;
#define PAIR(N, IQ, XQ)                                                        \
  struct N {                                                                   \
    int f(C auto) IQ { return 1; }                                             \
    int f(this N XQ, auto) { return 2; }                                       \
    int g(auto) IQ { return 1; }                                               \
    int g(this N XQ, C auto) { return 2; }                                     \
  }
PAIR(A, , &);
PAIR(B, &, &);
PAIR(K, const, const &);
PAIR(R, , &&);
PAIR(Q, &&, &&);
PAIR(P, const, const &&);
struct D : A {};
struct O {
  int operator()(C auto) { return 1; }
  int operator()(this O &, auto) { return 2; }
  int operator[](C auto) { return 1; }
  int operator[](this O &, auto) { return 2; }
  int operator+(C auto) { return 1; }
  int operator+(this O &, auto) { return 2; }
};
struct O2 {
  int operator()(auto) { return 1; }
  int operator()(this O2 &, C auto) { return 2; }
  int operator[](auto) { return 1; }
  int operator[](this O2 &, C auto) { return 2; }
  int operator+(auto) { return 1; }
  int operator+(this O2 &, C auto) { return 2; }
};
struct N1 { int operator+(C auto) { return 1; } int operator-(auto) { return 1; } };
int operator+(N1 &, auto) { return 4; }
int operator-(N1 &, C auto) { return 4; }
struct N2 { int operator+(C auto) const { return 1; } int operator-(auto) const { return 1; } };
int operator+(const N2 &, auto) { return 4; }
int operator-(const N2 &, C auto) { return 4; }
struct N3 { int operator+(C auto) && { return 1; } int operator-(auto) { return 1; } };
int operator+(N3 &&, auto) { return 4; }
int operator-(N3 &&, C auto) { return 4; }
#define P2(x, y) printf("%d%d ", x, y)
int main() {
  A a; B b; K k; const K ck; D d; O o; O2 o2; N1 n1; N2 n2; const N2 cn2;
  P2(a.f(0), a.g(0)); P2(b.f(0), b.g(0)); P2(d.f(0), d.g(0));
  P2(k.f(0), k.g(0)); P2(K{}.f(0), K{}.g(0)); P2(ck.f(0), ck.g(0));
  P2(R{}.f(0), R{}.g(0)); P2(Q{}.f(0), Q{}.g(0)); P2(P{}.f(0), P{}.g(0));
  P2(o(0), o2(0)); P2(o[0], o2[0]); P2(o + 0, o2 + 0);
  P2(n1 + 0, n1 - 0); P2(n2 + 0, n2 - 0); P2(N2{} + 0, N2{} - 0); P2(cn2 + 0, cn2 - 0);
  printf("%d%d\n", N3{} + 0, N3{} - 0);
}
