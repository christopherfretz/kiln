// [dcl.fct]/3: a lone unnamed void, also through a typedef, means no
// parameters; a dependent void type is not such a list.
//@ expect: compile-ok
//@ strict-diags
typedef void V;
void a(void);
void b(V);
template<class T> void t(T);
void (*fp)(void) = a;
int c() { b(); a(); return 0; }
