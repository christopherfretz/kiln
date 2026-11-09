// Coverage records of generic lambdas (WP-1083, spec/codegen.md §5.4
// "Instrumentation"): the conversion to a function pointer and the static
// invoker are implicit, a generic lambda's specializations of them too, so
// neither is instrumented nor gets a record; the call operator
// specializations do. The lambdas are in an inline function, so their
// names follow the Itanium ABI rather than kiln's local `$_N` numbering.
// A closure type is not a top-level declaration, so the call operator of
// a namespace-scope lambda that is never called at run time (here only
// in a `static_assert`) gets no zero record.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
auto a = [](int x) { return x; };
constexpr auto b = [](int x) { return x + 1; };
static_assert(b(1) == 2);
inline auto c = [](int x) { return x + 2; };
template <class F> constexpr int apply(F f) { return f(3); }
inline int f(int v) {
  int (*fp)(int) = [](auto x) { return x + 1; };
  long (*lp)(long) = [](auto x) { return x * 2; };
  int (*ip)(int) = [](int x) { return x - 1; };
  auto g = [](auto... xs) { return sizeof...(xs); };
  return fp(v) + lp(v) + ip(v) + g(1, 2) + apply([](auto y) { return y; });
}
int use(int v) { return f(v); }
