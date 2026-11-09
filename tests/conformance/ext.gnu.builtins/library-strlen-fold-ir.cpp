// clang's `EmitBuiltinExpr` first tries to evaluate a builtin call as an
// rvalue and emits the constant when that succeeds, also for a library
// builtin such as `strlen` (not a constant expression, but the evaluator
// folds it). kiln emits the library call. Found by the WP-871 fmt smoke:
// gtest's `strlen(prefix)` on a static const array gets a nonnull-attribute
// check under kiln but is the constant 7 under clang.
//@ expect: ir
//@ std: c++20
//@ xfail(kiln): WP-871 follow-up, library builtins (strlen) are not folded to constants in codegen
extern "C" __SIZE_TYPE__ strlen(const char *);

__SIZE_TYPE__ local_array() {
  static const char prefix[] = "std::__";
  return strlen(prefix);
}

__SIZE_TYPE__ literal() { return strlen("abc"); }
