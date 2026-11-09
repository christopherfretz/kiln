// Interface unit of module `cv` for carried-pointers-run.cpp
// (spec/modules.md §11.11 Part 2, WP-1047): constexpr pointers,
// references and member pointers, a class-type template argument holding
// a pointer, and specializations keyed on such values, all evaluated
// here, so a kind-2 module file carries them.
export module cv;
export int g = 1;
export int arr[4] = {10, 20, 30, 40};
export struct S {
  int a;
  int b;
  constexpr int f() const { return a; }
};
export S s{7, 8};
export struct B { int x = 5; };
export struct D : B { int y = 6; };
export D d;
export void fn() {}
export constexpr int *pg = &g;
export constexpr int *pa = &arr[2];
export constexpr int *pend = arr + 4;
export constexpr int *psb = &s.b;
export constexpr int *pn = nullptr;
export constexpr const int &r = g;
export constexpr int &ra = arr[1];
export constexpr void (*pf)() = &fn;
export constexpr int S::*pmb = &S::b;
export constexpr int (S::*pmf)() const = &S::f;
export constexpr int S::*pmn = nullptr;
export constexpr int D::*pdx = &D::x;
export struct Holder {
  int *p;
  int n;
};
export constexpr Holder h{&arr[3], 2};
export template <int *P> struct X {
  static int x;
  static constexpr int *self = P;
  static constexpr int v = 3;
};
template <int *P> int X<P>::x = 0;
export template <const int &R> struct XR {
  static int x;
  static constexpr const int *self = &R;
};
template <const int &R> int XR<R>::x = 0;
export template <int S::*M> struct XM {
  static int x;
};
template <int S::*M> int XM<M>::x = 0;
export template <Holder H> struct HW {
  static int x;
  static constexpr const Holder *obj = &H;
  static constexpr int v = H.n * 7;
};
template <Holder H> int HW<H>::x = 0;
export int *ax = &X<&g>::x;
export int *axa = &X<&arr[2]>::x;
export int *axr = &XR<g>::x;
export int *axm = &XM<&S::b>::x;
export int *ah = &HW<h>::x;
static_assert(X<&g>::self == &g && X<&arr[2]>::v == 3 && XR<g>::self == &g);
static_assert(HW<h>::v == 14 && HW<h>::obj->p == &arr[3]);
