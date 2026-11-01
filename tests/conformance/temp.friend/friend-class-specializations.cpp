// [class.friend]/3, [temp.friend]/1: a friend declaration naming a class
// template specialization (dependent or not, qualified or not, with a
// pack) befriends that specialization of each instantiation.
//@ expect: run
//@ exit: 0
namespace ns { template <class T> struct Z; }
template <class... T> struct P;
template <class T> struct X;
template <class T> struct Y;
template <class T> class A {
  friend struct X<T>;
  friend class Y<T*>;
  friend class ns::Z<const T>;
  friend struct X<char>;
  int priv = 1;
};
template <class... T> class Q {
  friend struct P<T...>;
  int priv = 2;
};
template <class T> struct X {
  int get(A<T>& a) { return a.priv; }
};
template <> struct X<char> {
  int get(A<int>& a) { return a.priv * 10; }
};
template <class T> struct Y;
template <class T> struct Y<T*> {
  int get(A<T>& a) { return a.priv * 100; }
};
namespace ns {
template <class T> struct Z {
  int get(A<int>& a) { return a.priv * 1000; }
};
}
template <class... T> struct P {
  static int get(Q<T...>& q) { return q.priv; }
};
struct Outer {
  template <class T> class In {
    friend struct X<In>;
    int priv = 5;
  };
};
template <> struct X<Outer::In<int>> {
  int get(Outer::In<int>& i) { return i.priv; }
};
int main() {
  A<int> a;
  Q<int, char> q;
  Outer::In<int> in;
  X<int> x;
  X<char> xc;
  Y<int*> y;
  ns::Z<const int> z;
  X<Outer::In<int>> xi;
  int r = x.get(a) + xc.get(a) + y.get(a) + z.get(a) + P<int, char>::get(q) + xi.get(in);
  return r == 1118 ? 0 : 1;
}
