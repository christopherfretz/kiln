// [temp.mem]/4: a specialization of a member function template does not
// override a virtual function of a base class, so a member template with
// the name and (deduced-default) signature of a base's virtual function,
// and another return type, is no invalid override; calls through the
// base still reach the base's function. In a class, a class template, a
// class template with a dependent base and a member class of a template.
//@ expect: run
//@ std: c++20
//@ stdout: 1 20 1 40 1 50 2 4 5
extern "C" int printf(const char *, ...);
struct B { virtual int schedule() const { return 1; } virtual int f() { return 10; } virtual ~B() = default; };
template <class T> struct D : B {
  template <class U = int> long schedule() const { return 2; }
  int f() override { return 20; }
};
template <class T> struct D2 : T {
  template <class U = int> long schedule() const { return 4; }
  int f() override { return 40; }
};
template <class T> struct Outer {
  struct In : B {
    template <class U = int> long schedule() const { return 5; }
    int f() override { return 50; }
  };
};
int main() {
  D<int> d; D2<B> d2; Outer<int>::In in;
  B& b = d; B& b2 = d2; B& b3 = in;
  printf("%d %d %d %d %d %d %ld %ld %ld\n", b.schedule(), b.f(), b2.schedule(), b2.f(), b3.schedule(), b3.f(), d.schedule(), d2.schedule(), in.schedule());
}
