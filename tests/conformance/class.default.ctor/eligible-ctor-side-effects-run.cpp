// [special]/6, [class.default.ctor]/3, [dcl.init.general]/7, /9,
// [expr.new]/23: when the selected constructor is non-trivial it runs,
// whether or not the class also declares a trivial (but ineligible, or
// differently-typed) one; when it is trivial, value-initialization
// zero-initializes. Every construction form (new, new[], default-init,
// value-init, arrays, members) counts its constructor calls, so a call
// skipped because of a class-wide "trivial" bit shows up (WP-1109).
//@ expect: run
//@ stdout: U<double> 9 9 9 9 9 9 9 9 9 calls=9
//@ stdout: U<int> 0 0 0 0
//@ stdout: S<int> 7 7 7 7 7 7 calls=6
//@ stdout: S<double> 0 0
//@ stdout: TD 5 5 5 5 calls=4
//@ stdout: C2 copies=1 K copies=1
#include <stdio.h>

static int calls;

template <class T> concept Big = sizeof(T) > 4;

// `U() = default` is less constrained than the satisfied `U() requires
// Big<T>`, so for U<double> only the non-trivial one is eligible.
template <class T> struct U {
  T t;
  U() = default;
  U() requires Big<T> : t(9) { ++calls; }
};

// Only the satisfied constructor is eligible.
template <class T> struct S {
  T t;
  S() requires Big<T> = default;
  S() requires (!Big<T>) : t(7) { ++calls; }
};

// A constructor template that is a default constructor.
struct TD {
  int v;
  template <class T = int> TD(T x = 5) : v(x) { ++calls; }
};

// Two eligible copy constructors; the non-const one is user-provided.
static int copies;
struct C2 {
  int v = 1;
  C2() = default;
  C2(const C2&) = default;
  C2(C2&) { ++copies; }
};

// The defaulted copy constructor is not eligible for K<false>.
template <bool B> struct K {
  int v = 1;
  K() = default;
  K(const K&) requires B = default;
  K(const K&) { ++copies; }
};

template <class T> struct Holder { T m; };

int main() {
  calls = 0;
  {
    U<double>* p = new U<double>;
    U<double>* q = new U<double>();
    U<double>* a = new U<double>[2];
    U<double> d;
    U<double> v{};
    Holder<U<double>> h{};
    U<double> arr[2];
    printf("U<double> %g %g %g %g %g %g %g %g %g calls=%d\n", p->t, q->t, a[0].t, a[1].t,
           d.t, v.t, h.m.t, arr[0].t, arr[1].t, calls);
    delete p;
    delete q;
    delete[] a;
  }
  {
    U<int>* q = new U<int>();
    U<int>* a = new U<int>[2]();
    U<int> v{};
    printf("U<int> %d %d %d %d\n", q->t, a[0].t, a[1].t, v.t);
    delete q;
    delete[] a;
  }
  calls = 0;
  {
    S<int>* p = new S<int>;
    S<int>* a = new S<int>[2]();
    S<int> d;
    S<int> v = S<int>();
    Holder<S<int>> h{};
    printf("S<int> %d %d %d %d %d %d calls=%d\n", p->t, a[0].t, a[1].t, d.t, v.t, h.m.t,
           calls);
    delete p;
    delete[] a;
  }
  {
    S<double>* q = new S<double>();
    S<double> v{};
    printf("S<double> %g %g\n", q->t, v.t);
    delete q;
  }
  calls = 0;
  {
    TD* p = new TD;
    TD* a = new TD[2];
    TD v{};
    printf("TD %d %d %d %d calls=%d\n", p->v, a[0].v, a[1].v, v.v, calls);
    delete p;
    delete[] a;
  }
  {
    C2 c;
    const C2 cc;
    C2 x(c);
    C2 y(cc);
    int c2 = copies;
    copies = 0;
    K<false> k;
    K<false> k2(k);
    K<true> t;
    K<true> t2(t);
    printf("C2 copies=%d K copies=%d\n", c2, copies + x.v - y.v + k2.v - t2.v);
  }
}
