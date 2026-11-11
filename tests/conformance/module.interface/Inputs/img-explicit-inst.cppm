export module imgei;
export namespace ei {
template <class T> struct X {
  T v;
  int get() const { return (int)v + 1; }
  int big() const;
  struct In { T w; int twice() const { return (int)w * 2; } };
};
template <class T> int X<T>::big() const { return (int)v * 100; }
extern template struct X<int>;
template struct X<int>;
template struct X<long>;
using XI = X<int>;
inline int use() { XI x{3}; return x.get(); }
template <class T, class U = T> struct Y { U u; U get() const { return u; } };
extern template struct Y<char>;
template struct Y<char>;
template <class T> struct Z { T v; int f() const; virtual int g() const { return 2; } };
template <class T> int Z<T>::f() const { return (int)v + 10; }
extern template struct Z<int>;
}
template struct ei::Z<int>;
