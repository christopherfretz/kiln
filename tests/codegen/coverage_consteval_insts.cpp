// Zero coverage records for instantiations used only in constant
// evaluation (WP-1083, spec/codegen.md §5.4 "Instrumentation"): clang
// hands every instantiated function definition to `EmitTopLevelDecl`
// as it is instantiated, which defers a zero record unless the function
// is emitted, in instantiation order among the top-level declarations.
// Constructors, destructors and immediate functions get none that way,
// nor does a std builtin such as `std::move`, which its calls never
// reference.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
namespace std {
template <class T> constexpr T &&move(T &x) noexcept { return static_cast<T &&>(x); }
}
template <class T> constexpr T sq(T x) { return x * x; }
template <class T> T plus1(T x) { return x + 1; }
template <class T> struct W {
  constexpr W() {}
  constexpr int size() const { return sizeof(T); }
  int unused() { return 0; }
};
consteval int ce(int x) { return x + 1; }
template <class T> consteval T cet(T x) { return x; }
inline int early() { return 0; }
namespace n {
inline int inner() { return plus1(2); }
static_assert(sq(3) == 9);
}
static_assert(W<char>{}.size() == 1);
constexpr long l = sq(4L);
static_assert(cet(1) == 1);
inline int late() { return plus1(2L) + sq(4u); }
int g(int v) { return sq(v) + ce(2) + std::move(v); }
