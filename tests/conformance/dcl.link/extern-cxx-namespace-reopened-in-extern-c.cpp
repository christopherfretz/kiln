// [dcl.link]/4: the innermost enclosing linkage-specification decides a
// declaration's language linkage. A namespace first defined inside
// `extern "C"` and reopened inside a nested `extern "C++"` gives its
// members C++ linkage there, so they overload the C-linkage function
// instead of conflicting with it ([dcl.link]/6 only merges C-linkage
// declarations). The shape is libstdc++'s bits/std_abs.h, whose
// `extern "C++" { namespace std { long abs(long); ... } }` follows glibc's
// `std::abs(int)` when <stdlib.h> is first included inside `extern "C" {`.
//@ expect: run
//@ strict-diags
//@ exit: 14
extern "C" {
namespace n { int f(int x) { return x + 1; } }
extern "C++" { namespace n { inline long f(long x) { return x + 10; } } }
}
namespace n { inline short f(short x) { return x + 100; } }
int main() { return n::f(1) + (int)n::f(1L) + n::f((short)1) - 100; }
