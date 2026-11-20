extern int counted2_runs;
struct Counted2 { Counted2(); int v; };
// Each declaration has its own `selectany`, so each unit defines the
// variables (zero-initialized, then default-initialized).
__attribute__((selectany)) extern int f;
extern __attribute__((selectany)) Counted2 obj;
