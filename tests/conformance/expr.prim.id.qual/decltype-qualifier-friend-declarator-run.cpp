// [class.friend], [expr.prim.id.qual]/1: a friend declarator-id may be
// qualified by a decltype-specifier, plain or naming a specialization, with
// a dependent operand inside a class template. Each friend grants access to
// the named member function. Reduced from real code.
//@ expect: run
//@ stdout: 42 7 3
class K;
struct A { int peek(const K&); };
A a;
class K {
  int secret = 42;
  friend int decltype(a)::peek(const K&);
};
int A::peek(const K& k) { return k.secret; }
template <class T> struct Box { int open(const T&); };
template <class T> class Locked {
  int v = 7;
  friend int decltype(Box<Locked>{})::open(const Locked&);
};
template <class T> int Box<T>::open(const T& t) { return t.v; }
template <int N> struct Bq { int g(); };
template <class T> class Deep {
  static constexpr int secret = 3;
  friend int decltype(Bq<sizeof(sizeof(T))>{})::g();
};
template <int N> int Bq<N>::g() { return Deep<int>::secret; }
int main() {
  K k;
  Locked<int> l;
  Box<Locked<int>> b;
  Bq<8> q;
  __builtin_printf("%d %d %d\n", a.peek(k), b.open(l), q.g());
}
