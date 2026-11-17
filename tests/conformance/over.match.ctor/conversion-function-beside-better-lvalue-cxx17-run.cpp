// [over.match.ctor]/1, CWG2327: a direct-initialization of 'B' from a 'U'
// with 'operator B() const' and 'operator B&()'. By the text the copy
// constructor (through 'operator B&') and the move constructor (through
// 'operator B') are indistinguishable ([over.ics.rank]/3.3). GCC 16 binds
// the move constructor's 'B&&' through 'operator B&' too (better object
// conversion), finds it not viable and calls the copy constructor; clang
// 22 lets 'operator B() const' initialize the object directly. Both
// accept, and kiln follows clang (docs/status/deviations.md). So do a
// rival lvalue conversion to a derived class and a conversion function
// template yielding a pointer. Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ xfail(gcc): GCC 16 calls the copy constructor through 'operator B&' (it binds the move constructor's 'B&&' through it)
//@ stdout: prvalue prvalue prvalue prvalue prvalue prvalue end
extern "C" int printf(const char *, ...);
struct B {
  B() {}
  B(const B &) { printf("copy "); }
  B(B &&) { printf("move "); }
};
struct D : B {};
B b0;
D d0;
struct U {
  operator B() const { printf("prvalue "); return B(); }
  operator B &() { printf("lvalue "); return b0; }
};
struct E {
  explicit operator B() const { printf("prvalue "); return B(); }
  operator B &() { printf("lvalue "); return b0; }
};
struct R {
  operator B() const { printf("prvalue "); return B(); }
  operator B &() { printf("lvalue "); return b0; }
  operator D &() { printf("derived "); return d0; }
};
struct P {
  operator B() const { printf("prvalue "); return B(); }
  operator B &() { printf("lvalue "); return b0; }
  template <class T> operator T *() { return nullptr; }
};
int main() {
  U u;
  B a(u);
  B b{u};
  B c = B(u);
  E e;
  B d(e);
  R r;
  B f(r);
  P p;
  B g(p);
  printf("end\n");
}
