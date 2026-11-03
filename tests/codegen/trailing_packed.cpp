// GNU extension: `packed` after a class body applies to the class
// (`<{ i8, i32 }>`, `align 1` accesses), also for a template.
struct A { char c; int i; } __attribute__((packed));
template <class T> struct TP { char c; T v; } __attribute__((packed));
int get(A *a) { return a->i; }
void set(A &a, int v) { a.i = v; }
A ga = {1, 2};
int tp(TP<int> &t) { return t.v; }
long long tl(TP<long long> t) { return t.v; }
