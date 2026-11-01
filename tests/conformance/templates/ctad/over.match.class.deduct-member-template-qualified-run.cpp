// [temp.mem], [over.match.class.deduct]/1: `Outer<int>::In x(...)` deduces
// a concrete specialization of the instantiated member template (not a
// dependent type), so the object is constructed and used at run time;
// likewise through a constructor template of the member.
//@ expect: run
//@ stdout: 1 2.5 4 7 9
extern "C" int printf(const char *, ...);
template<class T> struct Outer {
  template<class U> struct In {
    T t; U u;
    In(T a, U b) : t(a), u(b) {}
  };
  template<class U> struct H {
    U u;
    template<class V> H(V v, U w) : u(w + int(sizeof(V))) {}
  };
};
int main() {
  Outer<int>::In x(1, 2.5);
  auto y = Outer<long>::In(3L, 4);
  Outer<int>::H h('c', 6);
  Outer<int>::H h2(1.0, 1);
  printf("%d %g %d %d %d\n", x.t, x.u, y.u, h.u, h2.u);
  return 0;
}
