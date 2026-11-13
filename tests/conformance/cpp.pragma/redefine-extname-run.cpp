// [cpp.pragma]: '#pragma redefine_extname old new' gives the extern "C"
// function or variable 'old' the assembly name 'new' when it is declared
// after the pragma, or declared before it and neither defined nor used
// (clang ActOnPragmaRedefineExtname: clang's code generator has named a
// definition already, so it keeps its name). WP-1399.
//@ expect: run
//@ stdout: 1 2 2 5 6 6
#include <cstdio>
extern "C" int first_fn() { return 1; }
#pragma redefine_extname first_fn renamed_first
#pragma redefine_extname later_fn renamed_later
extern "C" int later_fn() { return 2; }
#pragma redefine_extname later_var renamed_var
extern "C" int later_var;
int later_var = 5;
extern "C" int declared_fn();
#pragma redefine_extname declared_fn renamed_declared
extern "C" int declared_fn() { return 6; }
extern "C" int renamed_later();
extern "C" int renamed_var;
extern "C" int renamed_declared();
int main() {
    std::printf("%d %d %d %d %d %d\n", first_fn(), later_fn(), renamed_later(), renamed_var,
                declared_fn(), renamed_declared());
}
