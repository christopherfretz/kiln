// [expr.new], [expr.const]/5.18 (C++26, P2747R2): a non-allocating placement
// new in a constant evaluation begins the lifetime of a new object in the
// storage its pointer designates: an object (or array of at least as many
// elements) of a similar type, whether within its lifetime, destroyed, or a
// heap allocation being reused; array forms included (clang 22's
// HandlePlacementNew rule).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
void *operator new[](std::size_t, void *p) noexcept;

struct S { int a, b; constexpr S(int x) : a(x), b(x + 1) {} };
struct B { int v = 1; };
struct D : B { int w = 2; };
union U { int i; float f; };
struct Counted {
  int *n;
  constexpr Counted(int *q) : n(q) { ++*n; }
  constexpr ~Counted() { --*n; }
};

constexpr int scalar() { int x = 1; int *p = new (&x) int(42); return *p + x; }
static_assert(scalar() == 84);

constexpr int replace_class() { S s(1); S *p = new (&s) S(5); return s.a + p->b; }
static_assert(replace_class() == 11);

constexpr int member() { S s(1); new (&s.b) int(7); return s.b; }
static_assert(member() == 7);

constexpr int base() { D d; new (static_cast<B *>(&d)) B{5}; return d.v + d.w; }
static_assert(base() == 7);

constexpr float union_member() { U u{.i = 1}; new (&u.f) float(2.5f); return u.f; }
static_assert(union_member() == 2.5f);

constexpr int value_init() { int x = 1; new (&x) int(); return x; }
static_assert(value_init() == 0);

// Reusing a heap object's storage after destroying it.
constexpr int reuse_heap() {
  void *v = new S(3);
  S *p = static_cast<S *>(v);
  p->~S();
  new (v) S(9);
  int r = p->a;
  delete p;
  return r;
}
static_assert(reuse_heap() == 9);

// Destroy and reconstruct an object with a non-trivial destructor.
constexpr int reconstruct() {
  int n = 0;
  {
    Counted c(&n);
    c.~Counted();
    new (&c) Counted(&n);
  }
  return n;
}
static_assert(reconstruct() == 0);

// Element by element into an array, and an array into an array object.
constexpr int elements() {
  int a[3] = {};
  for (int i = 0; i < 3; ++i) new (a + i) int(i * 2);
  return a[0] + a[1] + a[2];
}
static_assert(elements() == 6);

constexpr int whole_array() { int a[3] = {}; int *p = new (&a) int[3]{1, 2, 3}; return p[2] + a[0]; }
static_assert(whole_array() == 4);

// A runtime bound no larger than the array; the filler zeroes the rest of
// the new array.
constexpr int runtime_bound(int n) {
  int a[4] = {9, 9, 9, 9};
  new (&a) int[n]{7};
  return a[0] + a[1] + a[2];
}
static_assert(runtime_bound(3) == 7);
static_assert(runtime_bound(4) == 7);

constexpr int value_init_array() { int a[4] = {9, 9, 9, 9}; int n = 4; new (&a) int[n](); return a[3]; }
static_assert(value_init_array() == 0);

template <class T> constexpr T roundtrip(T v) { T x{}; T *p = ::new (&x) T(v); return *p; }
static_assert(roundtrip(3) == 3);
static_assert(roundtrip(2.0) == 2.0);
static_assert(roundtrip(B{4}).v == 4);
