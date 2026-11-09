// [class.virtual]/4, /8, [class.copy.assign]/6: valid twin of
// implicit-overrider.cpp. An implicit copy or move assignment operator may
// override a non-final virtual operator= with the same return type or a
// covariant one, a 'final' operator= taking another parameter type is not
// overridden, and a final class may have a final destructor. WP-1078.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags

struct D1;
struct D2;
struct B {
  virtual D1 &operator=(const D1 &);
  virtual D2 &operator=(D2 &&);
  virtual ~B();
};
struct D1 : B {};
struct D2 : B {};

struct D3;
struct C {
  virtual C &operator=(const D3 &);
  virtual C &operator=(D3 &&);
  virtual C &operator=(const C &) final;
};
struct D3 : C {};

template <class T> struct TB { virtual TB &operator=(const T &); };
template <class T> struct D5 : TB<D5<T>> {};
D5<int> d5;

struct G final { virtual ~G() final; };

D3 &assign(D3 &a, const D3 &b) { return a = b; }
