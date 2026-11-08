// [cpp.pragma]: '#pragma weak name' makes the function or variable 'name'
// weak (clang ActOnPragmaWeakID / ProcessPragmaWeak): a declaration before
// the pragma becomes weak; a pragma before any declaration applies to the
// first extern "C" function or variable of that name. An undefined weak
// symbol resolves to null at link time.
//@ expect: run
//@ stdout: 1 1 1 1 7 5
#include <cstdio>

extern "C" void after_decl();
#pragma weak after_decl

#pragma weak before_decl
extern "C" {
void before_decl();
extern int weak_var;
}
#pragma weak weak_var

int cxx_fn();
#pragma weak cxx_fn

#pragma weak defined_fn
extern "C" int defined_fn() { return 7; }

#pragma weak defined_var
extern "C" int defined_var;
int defined_var = 5;

int main() {
    std::printf("%d %d %d %d %d %d\n", after_decl == nullptr, before_decl == nullptr,
                &weak_var == nullptr, cxx_fn == nullptr, defined_fn(), defined_var);
}
