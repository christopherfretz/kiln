// [expr.static.cast]/14 (C++26, P2738R1): casts from `void *` back to the
// type the `void *` was made from, directly and after a store of the
// `void *` to memory (a member, an array element, a local read through a
// `void **`), give the same values at compile time and at run time.
//@ expect: run
//@ stdout: 2 2 1 3 4 1 1
//@ stdout: 2 2 1 3 4 1 1
extern "C" int printf(const char *, ...);
struct B { int b; };
struct D : B { int d; };
struct E {};
struct F {};
struct S : E { [[no_unique_address]] F f; int x; };
struct H { void *p; };
constexpr int derived(int k) { D d{{1}, 2 * k}; H h{&d}; return static_cast<D *>(h.p)->d; }
constexpr int base(int k) { D d{{k}, 2}; void *a[2] = {static_cast<B *>(&d), nullptr}; return static_cast<B *>(a[0])->b; }
constexpr int array(int k) { int a[3] = {1, 2, 3 * k}; void *p = &a; void **pp = &p; return (*static_cast<int(*)[3]>(*pp))[2]; }
constexpr int nested(int k) { int x = 4 * k; int *q = &x; H h{&q}; return **static_cast<const int *const *>(h.p); }
constexpr bool nua(int k) { S s{}; s.x = k; H h{&s.f}; return static_cast<F *>(h.p) == &s.f; }
constexpr bool empty_base(int k) { S s{}; s.x = k; H h{static_cast<E *>(&s)}; return static_cast<E *>(h.p) == static_cast<E *>(&s); }
constexpr int c[] = {derived(1), derived(1) / 2 * 2, base(1), array(1), nested(1), nua(1), empty_base(1)};
volatile int one = 1;
int main() {
  printf("%d %d %d %d %d %d %d\n", c[0], c[1], c[2], c[3], c[4], c[5], c[6]);
  int k = one;
  printf("%d %d %d %d %d %d %d\n", derived(k), derived(k) / 2 * 2, base(k), array(k), nested(k), nua(k),
         empty_base(k));
}
