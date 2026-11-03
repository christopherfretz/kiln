//@ args: -O1 -fno-strict-aliasing
// `-fno-strict-aliasing` turns TBAA off at every optimization level
// ([basic.lval]/11 is then not relied on; clang `CodeGenModule` creates no
// `CodeGenTBAA`): no `!tbaa`, `!tbaa.struct` or `!llvm.errno.tbaa`. At -O0
// no case carries TBAA either.
#define ON __attribute__((optnone))
struct S { int a; long b; virtual ~S(); };
ON long f(S* s, int* p, S* t) { s->a = *p; *p = 2; return s->b; }
struct T { int a; long b; };
ON void cp(T* a, T* b) { *a = *b; }
