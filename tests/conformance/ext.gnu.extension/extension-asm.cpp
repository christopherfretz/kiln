// [dcl.asm], GNU __extension__ (spec/parse.md §4): at namespace scope one or
// more __extension__ keywords before an asm-declaration are dropped (clang's
// ParseExternalDeclaration; libstdc++ writes __extension__ asm(...)), and the
// file-scope asm reaches the module.
//@ expect: run
//@ strict-diags
//@ args: -pedantic
//@ stdout: 42
extern "C" int printf(const char*, ...);
__extension__ asm(".pushsection .rodata\n.globl kiln_ext_asm_value\n"
                  "kiln_ext_asm_value: .long 42\n.popsection");
namespace N { __extension__ __extension__ asm(""); }
extern "C" { extern const int kiln_ext_asm_value; }
int main() { printf("%d\n", kiln_ext_asm_value); }
