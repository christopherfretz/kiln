// Mangling differential (crates/kiln_codegen/tests/mangle_vs_clang.rs):
// namespaces, classes, member functions, special members, operators.
namespace ns {
struct A {
  int x;
  A();
  A(int);
  A(const A &);
  A(A &&);
  ~A();
  A &operator=(const A &);
  A &operator=(A &&);
  int get() const;
  void vol() volatile;
  void cv() const volatile;
  void lref() &;
  void rref() &&;
  void clref() const &;
  operator int() const;
  operator bool();
  explicit operator const char *() const;
  static int count;
  static const int limit;
  static int sfn(int);
  static A make();
  struct In {
    int y;
    In();
    void f(A, In, A *, In *);
    static double z;
    struct Deeper {
      void g(const In &, const Deeper &);
    };
  };
};
A::A() {}
A::A(int) {}
A::A(const A &) {}
A::A(A &&) {}
A::~A() {}
A &A::operator=(const A &) { return *this; }
A &A::operator=(A &&) { return *this; }
int A::get() const { return 0; }
void A::vol() volatile {}
void A::cv() const volatile {}
void A::lref() & {}
void A::rref() && {}
void A::clref() const & {}
A::operator int() const { return 0; }
A::operator bool() { return false; }
A::operator const char *() const { return nullptr; }
int A::count = 3;
const int A::limit = 7;
int A::sfn(int) { return 0; }
A A::make() { return A(); }
A::In::In() {}
void A::In::f(A, A::In, A *, A::In *) {}
double A::In::z;
void A::In::Deeper::g(const A::In &, const A::In::Deeper &) {}

void take(A, A, const A, A &, const A &, A &&, A *, const A *, A **) {}
void take2(A::In, A::In::Deeper, A::In *, A::In::Deeper &) {}
int A::*pm_data;
int (A::*pm_fn)(int, double) const;
void pmtake(int A::*, int (A::*)() const, void (A::*)() volatile, int A::In::*) {}
void pmtake2(void (A::*)() &, void (A::*)() const &&, A *(A::*)()) {}
void fnptr(int (*)(A), A (*)(int, ...), void (&)(A &)) {}
void arrs(int (*)[3], A (&)[4][5], const A (*)[2]) {}

namespace inner {
struct B : A {
  B();
  ~B();
  void h(B, A, inner::B *);
};
B::B() {}
B::~B() {}
void B::h(B, A, inner::B *) {}
enum E { e0, e1 };
enum class F : long { f0 };
void en(E, F, E *, F &) {}
} // namespace inner
} // namespace ns

namespace {
struct Anon {
  void m();
  static int s;
};
void Anon::m() {}
int Anon::s;
void anonf(Anon) {}
} // namespace

void use_anon() {
  Anon a;
  a.m();
  anonf(a);
  Anon::s = 1;
}

struct Global {
  Global();
  ~Global();
  void f(Global, ns::A);
  static Global inst;
};
Global::Global() {}
Global::~Global() {}
void Global::f(Global, ns::A) {}

namespace std {
struct Thing {
  void f();
};
void Thing::f() {}
void stdfn(Thing, int) {}
} // namespace std

int local_holder() {
  struct L {
    int v;
    int get() const { return v; }
  };
  static int counter = 0;
  L l{counter};
  return l.get();
}
