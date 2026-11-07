// [temp.explicit]/10, [basic.def.odr]/8: an explicit instantiation
// definition of a polymorphic class emits its vtable, which odr-uses its
// virtual members, so an implicitly-declared virtual destructor is
// defined there (clang `ActOnExplicitInstantiation` ->
// `MarkVTableUsed(..., true)`), as is that of a member class it
// instantiates and of an explicitly instantiated member class (LLVM's
// `template class cl::opt<std::string>;` with `extern template` in the
// header). Classes 2 and 3 are not otherwise used here: their vtables
// reference the destructors only through the explicit instantiation.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: f=7 dtors=1 ok
#include <cstdio>
#include <string>
static int dtors = 0;
struct Counted {
  ~Counted() { ++dtors; }
};
struct Base {
  virtual ~Base();
  virtual int f() { return 0; }
};
Base::~Base() {}
template <class T> struct Opt : Base {
  T value;
  Counted c;
  int f() override { return 7; }
};
extern template struct Opt<std::string>;
template struct Opt<std::string>;
template <class T> struct Outer {
  struct Inner : Base {
    T t;
    std::string s;
  };
};
template struct Outer<int>::Inner;
template <class T> struct Outer2 {
  struct Inner2 : Base {
    std::string s;
    int f() override { return sizeof(T); }
  };
  T v;
};
template struct Outer2<long>;
int main() {
  Base *p = new Opt<std::string>;
  int f = p->f();
  delete p;
  std::printf("f=%d dtors=%d ok\n", f, dtors);
}
