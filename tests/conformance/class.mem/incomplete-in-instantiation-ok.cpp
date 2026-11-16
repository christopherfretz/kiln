// [class.mem.general]/8: a specialization instantiated inside a class's
// definition may use the class as a pointer, and its member functions,
// instantiated later, see the class complete; a non-empty pack expansion
// initializes an array of unknown bound's element type normally.
//@ std: c++20
template <class T> struct Y { T* p; int f() { return sizeof(T); } };
struct W {
  Y<W> y;
  int g() { return y.f(); }
  int n = sizeof(Y<W>);
};
template <class D> struct Crtp { int q() { return static_cast<D*>(this)->v; } };
struct Dv : Crtp<Dv> { int v; int r() { return q(); } };
template <class T> struct Outer { struct In { Outer* o; }; In i; };
Outer<int> oi;
template<class T> T&& mk();
template<class T, class... As> void tf() { T t(mk<As>()...); (void)t; }
void use() { tf<int>(); tf<int, int>(); }
