// [expr.typeid]/1-5, [class.cdtor]/5, [expr.const]/5: typeid yields one
// std::type_info object per type (cv and references dropped); a
// polymorphic glvalue's dynamic type, including during construction.
//@ expect: compile-ok
//@ strict-diags
namespace std {
class type_info {
public:
  constexpr bool operator==(const type_info &o) const { return this == &o; }
};
} // namespace std

struct A { constexpr virtual ~A() {} };
struct B : A {};
struct N { int n; };
struct M : N {};

// Static types: cv-qualifiers and references are dropped ([expr.typeid]/5).
static_assert(typeid(int) == typeid(const int));
static_assert(&typeid(int) == &typeid(volatile int &));
static_assert(!(typeid(int) == typeid(long)));
static_assert(&typeid(A) != &typeid(B));
static_assert(typeid(int[2]) == typeid(int[2]) && !(typeid(int[2]) == typeid(int[3])));

// A polymorphic glvalue has its dynamic type ([expr.typeid]/3); any
// other expression its static type, unevaluated.
constexpr bool dynamic() {
  B b;
  A &ra = b;
  const A *pa = &b;
  return typeid(ra) == typeid(B) && typeid(*pa) == typeid(B) && typeid(pa) == typeid(const A *);
}
static_assert(dynamic());
constexpr bool non_polymorphic() {
  M m;
  N &rn = m;
  return typeid(rn) == typeid(N) && typeid(m.n) == typeid(int);
}
static_assert(non_polymorphic());

// During construction the dynamic type is the class under construction.
struct C : A {
  bool was_c;
  constexpr C() : was_c(typeid(*this) == typeid(C)) {}
};
struct D : C {
  bool now_d = typeid(*this) == typeid(D);
};
constexpr bool cdtor() { D d; return d.was_c && d.now_d && typeid(d) == typeid(D); }
static_assert(cdtor());

// The same object for every evaluation.
constexpr const std::type_info *ti_b() { return &typeid(B); }
constexpr bool same() { return ti_b() == &typeid(B); }
static_assert(same());
