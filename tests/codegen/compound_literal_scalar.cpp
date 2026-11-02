// A scalar, vector or complex compound literal in a function is a load of
// a fresh temporary holding its initializer (clang
// `EmitCompoundLiteralLValue`, `VisitCompoundLiteralExpr`), also when it
// is materialized for a reference or subscripted.
typedef int v4 __attribute__((vector_size(16)));
int k() { return (int){5}; }
int h(int x) { return (int){x}; }
const int *p(int x) { const int &r = (const int){x}; return &r; }
v4 f(int x) { return (v4){x, 2, 3, 4}; }
int g(int x) { return ((v4){x, 2, 3, 4})[1]; }
v4 s(v4 a, int x) { return a + (v4){x}; }
_Complex double q(double x) { return (_Complex double){x}; }
double r(double x) { return __real__ (_Complex double){x}; }
struct P { int a, b; };
int pa(int x) { return ((P){x, 2}).b; }
void ign(int x) { (void)(int){x}; }
