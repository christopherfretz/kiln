// [basic.def.odr]/8 (WP-137): the vtables emitted for a constructor's
// class reference only defined functions: the class template's virtual
// members are instantiated and the implicit virtual destructors defined
// although nothing calls them directly.
//@ expect: run
//@ stdout: 9
extern "C" int printf(const char *, ...);
template <class T> struct B {
  virtual int f() { return T::value; }
  virtual ~B() {}
};
template <class T> struct V { virtual int v() { return 1; } virtual ~V() = default; };
template <class T> struct M : virtual V<T> { int m() { return 2; } };
struct X { static const int value = 3; };
struct D : B<X> { D() {} };
struct DV : M<int> { DV() {} };
B<X> *mk() { return new D; }
int local() {
  struct Loc : V<char> { int v() override { return 5; } };
  V<char> *p = new Loc;
  int r = p->v();
  delete p;
  return r;
}
int main() {
  B<X> *b = mk();
  delete b;
  V<int> *p = new DV;
  int r = p->v() + local();
  delete p;
  printf("%d\n", r + 3);
  return 0;
}
