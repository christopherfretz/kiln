// dynamic_cast to references (bad_cast), offset hints -2/-3, always-null
// casts; typeid of pointer/enum/array/function/incomplete-pointer types,
// array-subscript and conditional operands, most-derived and
// non-polymorphic operands. Throwing cases are not noexcept (no EH yet).
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char* name() const noexcept { return __name; }
  bool operator==(const type_info& o) const noexcept { return __name == o.__name; }
protected:
  const char* __name;
};
}
struct A { int a = 1; virtual ~A() noexcept {} };
struct B { long b = 2; virtual void fb() noexcept {} };
struct M1 : A {};
struct M2 : A {};
struct MM : M1, M2 {};
class Priv : A, public B {};
struct F final { virtual void f() noexcept {} };
struct G final : A {};
enum E { e0 };
enum class SE : short { x };
struct Inc;
struct C : A, B { int c = 3; };
C& rdown(A& a) { return dynamic_cast<C&>(a); }
const B& rcross(const A& a) { return dynamic_cast<const B&>(a); }
MM* amb(A* a) noexcept { return dynamic_cast<MM*>(a); }
Priv* priv(B* b) noexcept { return dynamic_cast<Priv*>(b); }
F* never(A* a) noexcept { return dynamic_cast<F*>(a); }
F& rnever(A& a) { return dynamic_cast<F&>(a); }
G* fin(A* a) noexcept { return dynamic_cast<G*>(a); }
const char* n1() noexcept { return typeid(C*).name(); }
const char* n2() noexcept { return typeid(const C* const*).name(); }
const char* n3() noexcept { return typeid(E).name(); }
const char* n4() noexcept { return typeid(SE*).name(); }
const char* n5() noexcept { return typeid(Inc*).name(); }
const char* n6() noexcept { return typeid(int[3]).name(); }
const char* n7() noexcept { return typeid(void(int)).name(); }
const char* n8() noexcept { return typeid(const C&).name(); }
const char* n9() noexcept { return typeid(volatile int*).name(); }
bool same(A* a, A* b) { return typeid(*a) == typeid(*b); }
const char* arr(A* as) { return typeid(as[1]).name(); }
const char* nonpoly(B* b, int i) noexcept { return typeid(i + 1).name(); }
const char* local() noexcept { C c; return typeid(c).name(); }
const char* cond(bool k, A* a, A& r) { return typeid(k ? *a : r).name(); }
