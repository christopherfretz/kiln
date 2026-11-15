// Debug info ([dcl.typedef]/1, [namespace.udecl]): a function declared
// through a function typedef, an alias or `decltype`, named by a
// using-declaration and not defined in the unit, gets a declaration-only
// DISubprogram whose `type:` is a DISubroutineType, as clang's
// `getFunctionFwdDeclOrStub` writes it: the return and parameter types
// keep their sugar, `...` stays, and `noreturn` carried by the typedef
// gives DIFlagNoReturn. kiln wrote the DW_TAG_typedef as `type:`, which
// LLVM's verifier rejects ("invalid subroutine type"), dropping the
// module's debug information. Reduced from real code.
//@ args: -g
namespace N {
typedef int Fn(int);
typedef void NR(int) __attribute__((noreturn));
typedef const char *Str;
typedef Str SFn(Str, ...);
using AF = long(long);
int c(const int, int[]) noexcept;
Fn h;
NR nr;
SFn v;
AF af;
decltype(c) dc;
}
using N::h;
using N::nr;
using N::v;
using N::af;
using N::c;
using N::dc;
int main() { return h(1); }
