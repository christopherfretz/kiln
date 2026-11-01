// [dcl.asm]: GCCAsmStmt (outputs then inputs; register inputs are
// converted to prvalues, memory inputs stay lvalues) and FileScopeAsmDecl
// in clang's dump, also in an uninstantiated template pattern.
//@ expect: ast
//@ oracle: clang
asm("nop");
template <class T> void dno(T& value) { asm volatile("" : "+r,m"(value) : : "memory"); }
template <class T> void dnc(const T& value) { asm volatile("" : : "r,m"(value) : "memory"); }
int f(int x) { int o; asm("mov %w0, %w1" : [o] "=r"(o) : "m"(x), "i"(42), "r"(x + 1)); asm("nop"); return o; }
void g(const int& c) { asm volatile("" : : "r,m"(c) : "memory"); }
