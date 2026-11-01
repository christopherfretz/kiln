// [temp.spec.partial.member]/1, [class.ctor.general]/1: in an out-of-line
// definition, `X<T*>::X(` names the constructor of the partial
// specialization (the qualifier is not parsed ahead as a type).
//@ expect: run
//@ strict-diags
template <class> struct X;
template <class T> struct X<T*> { int v; X(); X(const X&); ~X(); };
int dtors = 0;
template <class T> X<T*>::X() : v(sizeof(T)) {}
template <class T> X<T*>::X(const X& o) : v(o.v + 1) {}
template <class T> X<T*>::~X() { ++dtors; }
template <class R, class... A> struct X<R(A...)> { int n; X(); };
template <class R, class... A> X<R(A...)>::X() : n(sizeof...(A)) {}
int main() {
  int r;
  {
    X<long*> a;
    X<long*> b(a);
    X<int(char, long, short)> f;
    r = (a.v == 8 && b.v == 9 && f.n == 3) ? 0 : 1;
  }
  return r + (dtors == 2 ? 0 : 2);
}
