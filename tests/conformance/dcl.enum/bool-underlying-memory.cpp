// [dcl.enum]/5, [class.bit]/4: an object of an enumeration whose
// underlying type is `bool` is stored as a `bool` (an `i8` holding 0 or 1,
// clang `EmitToMemory`/`EmitFromMemory`), whether it is a member, a
// bit-field, an array element, a global with a constant or dynamic
// initializer, or a constexpr object whose value is emitted; an atomic
// load or store of it (`__atomic_load_n` of an unscoped one, `__atomic_load`
// of a scoped one, as `std::atomic<E>` does) goes through the same byte.
// The shape is a downstream `enum class Policy : bool`.
//@ expect: run
//@ stdout: 1 0 1 1 0 1
//@ stdout: 5 1 9 0 7 1
//@ stdout: 1 0 1 0 1
//@ stdout: 1 1 0 1 1
//@ stdout: 1 0 1 1 1
extern "C" int printf(const char *, ...);
extern "C" void *memset(void *, int, unsigned long);

enum class P : bool { NO, YES };
enum U : bool { UN, UY };

struct M {
  char c;
  P p;
  U u;
  P arr[3];
};

struct B {
  unsigned lo : 3;
  P p : 1;
  unsigned mid : 4;
  U u : 1;
  unsigned hi : 7;
};

P g_yes = P::YES;
P g_arr[4] = {P::NO, P::YES, P::YES, P::NO};
const M g_m = {'x', P::YES, UN, {P::YES, P::NO, P::YES}};
constexpr P k_yes = P::YES;
constexpr P k_list[2] = {P::NO, P::YES};
__attribute__((noinline)) P dyn() { return P::YES; }
P g_dyn = dyn();

__attribute__((noinline)) int read(const P *p) { return static_cast<int>(*p); }
__attribute__((noinline)) void write(P *p, int v) { *p = static_cast<P>(v); }
__attribute__((noinline)) int byte(const void *p) {
  return *static_cast<const unsigned char *>(p);
}

int main() {
  M m;
  memset(&m, 0xff, sizeof m);
  m.p = P::YES;
  m.u = UN;
  m.arr[0] = P::NO;
  m.arr[1] = static_cast<P>(m.c);
  write(&m.arr[2], 4);
  printf("%d %d %d %d %d %d\n", byte(&m.p), byte(&m.u), byte(&m.arr[1]),
         byte(&m.arr[2]), read(&m.arr[0]), m.arr[2] == P::YES);

  B b{5, P::YES, 9, UN, 7};
  P q = b.p;
  printf("%d %d %d %d %d %d\n", b.lo, static_cast<int>(q), b.mid, (int)b.u,
         b.hi, b.p == P::YES);
  b.p = P::NO;
  b.u = static_cast<U>(b.mid);
  printf("%d %d %d %d %d\n", (int)b.u, (int)b.p, b.lo == 5, b.mid == 9 ? 0 : 1,
         b.hi == 7);

  printf("%d %d %d %d %d\n", byte(&g_yes), byte(&g_arr[2]), byte(&g_arr[3]),
         byte(&g_m.p), byte(&g_dyn));

  U cell = UN;
  __atomic_store_n(&cell, UY, __ATOMIC_SEQ_CST);
  U seen = __atomic_load_n(&cell, __ATOMIC_ACQUIRE);
  U old = __atomic_exchange_n(&cell, UN, __ATOMIC_SEQ_CST);
  U expect = UN;
  bool swapped = __atomic_compare_exchange_n(&cell, &expect, UY, false,
                                             __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
  P pcell = P::NO, pin = k_yes, pout = P::NO;
  __atomic_store(&pcell, &pin, __ATOMIC_RELEASE);
  __atomic_load(&pcell, &pout, __ATOMIC_ACQUIRE);
  printf("%d %d %d %d %d\n", static_cast<int>(seen), byte(&k_list[0]),
         old == UY, swapped && cell == UY, pout == P::YES && byte(&pcell) == 1);
}
