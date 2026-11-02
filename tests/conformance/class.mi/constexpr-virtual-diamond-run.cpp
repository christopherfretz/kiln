// [class.mi]/4, [class.base.init]/7,13, [class.cdtor]/4, [class.dtor]/13,
// [conv.ptr]/3, [expr.dynamic.cast] (C++26, P3533R2): a diamond with a
// polymorphic virtual base, built and destroyed during constant evaluation
// and again at run time, with the same results. The most derived
// constructor initializes the shared virtual base first and the
// intermediate constructors skip its initializer; virtual calls during a
// base's construction and destruction dispatch to that base; the virtual
// base is one subobject reached from every path; a virtual call through it
// dispatches on the complete object; dynamic_cast goes down and across;
// destruction runs in reverse, the virtual base last. The values were
// checked against clang 19's run time with `constexpr` removed (no clang
// implements P3533).
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: run
//@ strict-diags
//@ stdout: A1B22C1D dc1b2a1 16
//@ stdout: A1B22C1D dc1b2a1 16
//@ stdout: 1171545 1171545
extern "C" int printf(const char *, ...);

struct Log {
  char buf[32]{};
  int n = 0;
  constexpr void put(char c) { buf[n++] = c; }
};
struct A {
  Log *l;
  int a;
  constexpr A(Log *l, int x) : l(l), a(x) { l->put('A'); l->put(char('0' + who())); }
  constexpr virtual int who() const { return 1; }
  constexpr virtual ~A() { l->put('a'); l->put(char('0' + who())); }
};
struct B : virtual A {
  int b;
  constexpr B(Log *l) : A(l, 10), b(2) {
    l->put('B');
    l->put(char('0' + who()));
    l->put(char('0' + static_cast<A *>(this)->who()));
  }
  constexpr int who() const override { return 2; }
  constexpr ~B() override { l->put('b'); l->put(char('0' + who())); }
};
struct C : virtual A {
  int c;
  constexpr C(Log *l) : A(l, 20), c(3) { l->put('C'); l->put(char('0' + static_cast<A &>(*this).who())); }
  constexpr ~C() override { l->put('c'); l->put(char('0' + who())); }
};
struct D : B, C {
  int d;
  constexpr D(Log *l) : A(l, 5), B(l), C(l), d(4) { l->put('D'); }
  constexpr int who() const override { return 4; }
  constexpr ~D() override { l->put('d'); }
};

// Construction then destruction order, with a space between them.
constexpr Log lifetime() {
  Log l;
  {
    D d(&l);
    l.put(' ');
  }
  return l;
}

constexpr int paths(int seven) {
  Log l;
  int r;
  {
    D d(&l);
    A *pa = static_cast<C *>(&d);
    A *pb = static_cast<B *>(&d);
    r = (pa == pb) * 1000 + pa->a * 100 + pa->who() * 10 + d.b + d.c;
    pa->a = seven;
    r += static_cast<B &>(d).a * 10000;
    D *back = dynamic_cast<D *>(pa);
    C *cross = dynamic_cast<C *>(static_cast<B *>(&d));
    r += (back == &d) * 100000 + (cross == static_cast<C *>(&d)) * 1000000;
  }
  return r;
}

constexpr Log ct_log = lifetime();
constexpr int ct_paths = paths(7);
static_assert(ct_paths == 1171545);

int main(int argc, char **) {
  printf("%s %d\n", ct_log.buf, ct_log.n);
  Log rt = lifetime();
  printf("%s %d\n", rt.buf, rt.n);
  printf("%d %d\n", ct_paths, paths(argc + 6));
}
