// The skipped region of a discarded `if constexpr` branch
// ([stmt.if]/2; WP-1083, spec/codegen.md §5.4 "Instrumentation"): clang
// instantiates the branch as an empty compound statement over the
// written branch's begin and end, so the region ends after the branch's
// last token (a `}`, a `return`'s operand), here in a function template,
// a generic lambda with an init-capture and an `else if constexpr`.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
template <class T> int f(T a) {
  if constexpr (sizeof(T) == 4) {
    return 1;
  } else {
    int x = 2;
    return x;
  }
}
template <class T> int g(T a) {
  if constexpr (sizeof(T) == 8) {
    return 1;
  } else if constexpr (sizeof(T) == 2)
    a++;
  else
    while (a > 0) a--;
  return 0;
}
template <class T> int k(T a) {
  auto l = [v = a](auto b) {
    if constexpr (sizeof(b) == 4)
      return v;
    else
      return 2;
  };
  return l(a);
}
int h() { return f(1) + f(1L) + g(1) + k(1); }
