// GNU `alias` (spec/codegen.md "Aliases and ifuncs"): an `extern "C"`
// alias that names a C++ function before its definition declares the
// target with the alias declaration's attributes (clang
// `GetOrCreateLLVMFunction` sets them once, from the creating
// declaration; an `extern "C"` declaration has no strict return, so no
// `noundef` on the return), and the later definition keeps them.
extern "C" int a(int) __attribute__((alias("_Z1fi")));
int f(int x) { return x; }
int use() { return a(1) + f(2); }
