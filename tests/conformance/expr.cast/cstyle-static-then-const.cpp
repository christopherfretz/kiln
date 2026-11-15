// [expr.cast]/4.3: a C-style cast from a pointer to const derived to a pointer to a base is a
// static_cast followed by a const_cast, in constant evaluation too: the static_cast adjusts the
// address. When that interpretation is ill-formed (an ambiguous base, a downcast through a
// virtual base) the cast is ill-formed; it does not fall back to a reinterpret_cast
// ([expr.cast]/4: "even if a cast resulting from that interpretation is ill-formed").
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -Wno-inaccessible-base
//@ strict-diags
//@ diag: 27 error "ambiguous conversion from derived class"
//@ diag: 28 error "via virtual base"
struct A { int a = 1; };
struct B { int b = 2; };
struct D : A, B { virtual void f() {} int d = 3; };
constexpr D d;
static_assert((const B*)&d == static_cast<const B*>(&d));
static_assert(((B*)&d)->b == 2);
static_assert((D*)(B*)&d == &d);
static_assert(&(B&)d == static_cast<const B*>(&d));
constexpr int g() { D x; const D* p = &x; B* q = (B*)p; q->b = 5; return x.b; }
static_assert(g() == 5);

struct V { int v; };
struct E : virtual V {};
struct F : V, E {};
void h(const F* f, const V* v, const E* e) {
  (void)(V*)f;
  (void)(E*)v;
  (void)(V*)e;
}
