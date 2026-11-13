// [dcl.enum]/5, [expr.static.cast]/10: the `bool` representation of an
// enumeration whose underlying type is `bool` under optimization, where
// its loads carry `!range !{i8 0, i8 2}` as a `bool`'s do (clang
// `getRangeForLoadFromType`, `hasBooleanRepresentation`) and the `i1`
// values reach comparisons, selects and switches folded by LLVM. The shape
// is a downstream Handler (`Policy : bool`
// member tested with `==`).
//@ expect: run
//@ args: -O2
//@ stdout: 3 1 1 0 2
//@ stdout: 1 1 0 1 1
extern "C" int printf(const char *, ...);

enum class P : bool { NO, YES };
enum U : bool { UN, UY };

struct Handler {
  const P emit;
  U flag;
  P bits : 1;
  int run() const;
};
__attribute__((noinline)) int Handler::run() const {
  if (emit == P::YES) return 3;
  return 1;
}
__attribute__((noinline)) P from_int(int i) { return static_cast<P>(i); }
__attribute__((noinline)) int sw(const P *p) {
  switch (*p) {
  case P::NO: return 1;
  case P::YES: return 2;
  }
  return 0;
}
__attribute__((noinline)) U load(U *u) { return __atomic_load_n(u, __ATOMIC_ACQUIRE); }

int main(int argc, char **) {
  Handler a{P::YES, UY, P::NO}, b{P::NO, UN, P::YES};
  P arr[2] = {from_int(argc + 1), from_int(argc - 1)};
  printf("%d %d %d %d %d\n", a.run(), b.run(), static_cast<int>(arr[0]),
         static_cast<int>(arr[1]), sw(&arr[0]));
  U cell = a.flag;
  printf("%d %d %d %d %d\n", (int)load(&cell), b.bits == P::YES,
         (int)a.bits, from_int(256) == P::YES, a.flag + b.flag);
}
