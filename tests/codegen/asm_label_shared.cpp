// [dcl.asm] (GNU asm labels): declarations of different types whose
// labels name one symbol are one function or variable (clang
// `GetOrCreateLLVMFunction` / `GetOrCreateLLVMGlobal` return the entry
// whatever its type), called or loaded with each declaration's own
// type; a definition of another type replaces the declaration at the
// end of the module.
void a(int) asm("lbl");
void b(long) asm("lbl");
void use() { a(1); b(2); }
void b(long) {}
void use2() { a(3); b(4); }
int v1 asm("vl");
extern long v2 asm("vl");
long rd() { return v2 + v1; }
extern long w2 asm("wl");
long r0() { return w2; }
int w1 asm("wl") = 3;
int r1() { return w1; }
