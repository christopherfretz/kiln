// A local reference whose initializer is a constant expression
// ([expr.const]/4, [dcl.init.ref]/5) stores the constant address (clang
// `EmitAutoVarInit` via `ConstantLValueEmitter`: a byte-offset GEP), and a
// non-constant one binds the evaluated glvalue.
struct E { int y; int z[4]; };
int ga[4];
E e;
E ea[2];
int y() { int &lr = ga[3]; return lr; }
int z() { int &lr = e.z[2]; const int &cr = ea[1].y; return lr + cr; }
int nc(int i) { int &lr = ga[i]; return lr; }
int st() { static int s; int &lr = s; return lr; }
int tmp() { const int &r = 5; return r; }
