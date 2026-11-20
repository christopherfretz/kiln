// [dcl.link]/2 and [lex.string]/7: the unevaluated-string of a
// linkage-specification is the concatenation of adjacent string
// literals, so `extern "C" ""` gives C language linkage and `extern "C"
// "++"` C++ linkage. The C function defined in the global namespace is
// the one declared in A ([dcl.link]/6), so the program links (WP-1745).
//@ expect: run
//@ strict-diags
//@ exit: 0
namespace A { extern "C" "" int kiln_cat_f(int); }
extern "C" "++" { int kiln_cat_g(int x) { return x + 1; } }
extern "C" int kiln_cat_f(int x) { return x * 2; }
int main() { return A::kiln_cat_f(3) + kiln_cat_g(1) - 8; }
