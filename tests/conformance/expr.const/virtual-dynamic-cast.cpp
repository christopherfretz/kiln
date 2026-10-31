// [expr.dynamic.cast]/7-9, [class.cdtor]/6, [expr.const]/5: dynamic_cast
// in constant evaluation: downcasts, cross-casts, to `void*`, null
// operands, reference casts, and the dynamic type of an object under
// construction or destruction.
//@ expect: compile-ok
//@ strict-diags
namespace std {
class type_info {
public:
  constexpr bool operator==(const type_info &o) const { return this == &o; }
};
} // namespace std

struct A { int a = 1; constexpr virtual int f() const { return 1; } };
struct B : A { int b = 2; };
struct X { int x = 3; constexpr virtual ~X() {} };
struct C : B, X { int c = 4; };
struct D : C {};

// Downcast, cross-cast, failed downcast, void*, null.
constexpr bool down() { C c; A *pa = &c; return dynamic_cast<C *>(pa) == &c; }
static_assert(down());
constexpr bool cross() {
  C c;
  A *pa = &c;
  X *px = dynamic_cast<X *>(pa);
  return px == static_cast<X *>(&c) && px->x == 3;
}
static_assert(cross());
constexpr bool miss() { B b; A *pa = &b; return dynamic_cast<C *>(pa) == nullptr; }
static_assert(miss());
constexpr bool to_void() {
  C c;
  X *px = &c;
  return dynamic_cast<void *>(px) == static_cast<void *>(&c) &&
         dynamic_cast<const void *>(static_cast<const A *>(&c)) == &c;
}
static_assert(to_void());
constexpr bool null() { A *pa = nullptr; return dynamic_cast<B *>(pa) == nullptr; }
static_assert(null());
constexpr bool up() { D d; X *px = &d; return dynamic_cast<A *>(px) == static_cast<A *>(&d); }
static_assert(up());

// Reference casts.
constexpr bool ref() {
  C c;
  A &ra = c;
  X &rx = dynamic_cast<X &>(ra);
  C &rc = dynamic_cast<C &>(rx);
  return rx.x == 3 && &rc == &c;
}
static_assert(ref());

// During construction the dynamic type is the class under construction
// ([class.cdtor]/6).
constexpr bool is_g(const A *p);
struct F : A {
  bool saw_g;
  constexpr F() : saw_g(is_g(this)) {}
  constexpr ~F() { saw_g = is_g(this); }
};
struct G : F { bool g = is_g(this); };
constexpr bool is_g(const A *p) { return dynamic_cast<const G *>(p) != nullptr; }
constexpr bool cdtor() { G g; return !g.saw_g && g.g; }
static_assert(cdtor());

// A constant pointer from dynamic_cast.
constexpr C gc;
constexpr const X *gx = dynamic_cast<const X *>(static_cast<const A *>(&gc));
static_assert(gx == &gc && gx->x == 3);
