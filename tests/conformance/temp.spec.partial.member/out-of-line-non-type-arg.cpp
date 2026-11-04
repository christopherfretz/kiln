// [temp.spec.partial.member], [temp.dep.type]/1.2: in the out-of-line
// member definitions of a partial specialization with a non-type argument
// (bslalg's `DequeIterator<VALUE_TYPE, 1>`), `DI<V, 1>` written in the return
// type or a parameter names the current instantiation, so it matches the
// in-class declarations' injected-class-name `DI`.
//@ expect: run
//@ exit: 1
template <class V, int N> class DI;
template <class V> class DI<V, 1>;
template <class V, int N> class DI { V *p; public: DI operator+(long) const; };
template <class V> class DI<V, 1> {
  V *p;
public:
  DI() {}
  explicit DI(V *q) : p(q) {}
  DI operator+(long) const;
  DI operator-(long) const;
  long operator-(const DI &rhs) const;
};
template <class V> inline DI<V, 1> DI<V, 1>::operator+(long o) const { return DI<V, 1>(p + o); }
template <class V> inline DI<V, 1> DI<V, 1>::operator-(long o) const { return DI<V, 1>(p - o); }
template <class V> inline long DI<V, 1>::operator-(const DI<V, 1> &r) const { return p - r.p; }
int main() { int a[3]; DI<int, 1> x(a), y = x + 2; return (int)((y - 1) - x); }
