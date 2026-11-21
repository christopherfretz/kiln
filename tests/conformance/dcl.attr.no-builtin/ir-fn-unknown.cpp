// `-fno-builtin-<name>` keeps only the names clang knows as library
// builtins (`Builtin::Context::isBuiltinFunc`): `bcmp` and `alloca`
// (GNU-mode builtins) give `"no-builtin-<name>"`; `memmem`, `foo`, a
// `std::` builtin without its `std-` prefix (`move`) and a plain one with
// it (`std-memcpy`) give nothing. spec/codegen.md "no-builtin", WP-1863.
//@ expect: ir
//@ args: -fno-builtin-bcmp -fno-builtin-alloca -fno-builtin-memmem -fno-builtin-foo -fno-builtin-move -fno-builtin-std-memcpy
extern "C" int g(int);

extern "C" int f(int x) { return g(x) + 1; }
