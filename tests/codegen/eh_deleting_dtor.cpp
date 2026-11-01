// [class.dtor]/16, [expr.delete]/6: a deleting destructor (D0) calls the
// complete-object destructor and then the deallocation function, which
// still runs when that destructor throws (clang's normal-and-EH
// `CallDtorDelete` cleanup); global sized, class-specific sized and
// unsized deallocation functions, a derived class, a noexcept destructor.
namespace std { using size_t = decltype(sizeof 0); }
struct S {
  virtual ~S() noexcept(false);
  int x;
};
S::~S() noexcept(false) { if (x) throw 1; }

struct C {
  virtual ~C() noexcept(false);
  void operator delete(void *, std::size_t);
};
C::~C() noexcept(false) {}

struct U {
  virtual ~U() noexcept(false);
  void operator delete(void *);
  long y;
};
U::~U() noexcept(false) { throw 2; }

struct D : S {
  ~D() noexcept(false) override;
  char c[3];
};
D::~D() noexcept(false) {}

struct N {
  virtual ~N();
};
N::~N() {}

void kill(S *s, C *c, U *u, D *d, N *n) {
  delete s;
  delete c;
  delete u;
  delete d;
  delete n;
}
