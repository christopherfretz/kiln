// The closure types of lambdas in a function with internal linkage have
// no mangling number, so clang names them `$_N` (ItaniumMangle
// `getAnonymousStructId`). It allocates N in the order names are first
// mangled, not in source order. `apply<lambda>` is handed to CodeGen at
// the end of the TU, before the deferred static `g` is emitted, so the
// lambda passed to it is `$_0` and `less` is `$_1`. kiln numbers them in
// source order. The names are internal (no ABI effect), but
// `typeid(...).name()` shows them. Found in LLVM's ScalarEvolution.cpp
// (`GroupByComplexity`), WP-782.
//@ expect: run
//@ stdout: ZL1giE3$_1 ZL1giE3$_0 1
#include <cstdio>
#include <typeinfo>

template <class F> const char *apply(F) { return typeid(F).name(); }

static void g(int n) {
  auto less = [&](int a, int b) { return a + n < b; };
  const char *ln = typeid(less).name();
  const char *sn = apply([](int a, int b) { return a < b; });
  std::printf("%s %s %d\n", ln, sn, less(1, 2));
}

int main() { g(0); }
