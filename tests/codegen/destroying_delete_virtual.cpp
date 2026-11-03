// [expr.delete]/7.1, [class.dtor]/16: a destroying operator delete
// reached through a virtual destructor. The delete-expression calls the
// deleting destructor (D0) virtually, never devirtualized (clang
// `EmitDestroyingObjectDelete` -> `emitVirtualObjectDelete`); a D0 whose
// class's deallocation function is a destroying one calls only that
// function, passing `this` converted to its first parameter and the size
// of the destructor's class, not the complete destructor (clang
// `EnterDtorCleanups`). Covers a destroying delete with and without the
// size and alignment parameters, one inherited from a base (reached from
// a non-primary base through a thunk), and a derived class whose own
// usual delete hides the base's destroying one (spec/codegen.md
// "Landed (WP-457)").
namespace std {
using size_t = decltype(sizeof 0);
enum class align_val_t : size_t {};
struct destroying_delete_t {
  explicit destroying_delete_t() = default;
};
inline constexpr destroying_delete_t destroying_delete{};
} // namespace std

struct A {
  virtual ~A();
  void operator delete(A *, std::destroying_delete_t);
};
A::~A() {}

struct S {
  virtual ~S();
  void operator delete(S *, std::destroying_delete_t, std::size_t);
};
S::~S() {}

struct alignas(64) L {
  virtual ~L();
  void operator delete(L *, std::destroying_delete_t, std::size_t,
                       std::align_val_t);
};
L::~L() {}

struct X {
  virtual ~X();
  int x;
};
X::~X() {}

// Inherited: D's deleting destructor calls B's destroying delete with
// `this` converted to `B*` (a non-primary base: an offset GEP).
struct B {
  virtual ~B();
  void operator delete(B *, std::destroying_delete_t);
};
B::~B() {}
struct D : X, B {
  ~D() override;
  long d;
};
D::~D() {}

// A derived class's own usual delete hides the base's destroying one.
struct U : A {
  ~U() override;
  void operator delete(void *);
};
U::~U() {}

void del_a(A *p) { delete p; }
void del_s(S *p) { delete p; }
void del_l(L *p) { delete p; }
void del_b(B *p) { delete p; }
void del_d(D *p) { delete p; }
void del_u(U *p) { delete p; }
void del_ua(A *p) { delete static_cast<U *>(p); }
