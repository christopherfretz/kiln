// new-expressions in conditionally evaluated operands ([expr.cond],
// [expr.log.and]): the operator delete cleanup's active flag and saves.
int *a(bool c) { return c ? nullptr : new int(4); }
int *b(bool c, bool d) { return c ? (d ? new int(1) : nullptr) : nullptr; }
int *e(bool c, int n) { return c ? new int[n]{1,2} : nullptr; }
bool f(bool c) { return c || new int(4); }
int *h(bool c) { return c ? new int : nullptr; }
void *operator new(unsigned long, short, char);
void operator delete(void *, short, char) noexcept;
void *operator new[](unsigned long, short, char);
void operator delete[](void *, short, char) noexcept;
int *pa(bool c, short s, char ch) { return c ? new (s, ch) int(1) : nullptr; }
int *pb(bool c, int n) { return c ? new (short(n), 'x') int[n]{1} : nullptr; }
struct alignas(32) A { int x[8]; };
A *pd(bool c) { return c ? new A{} : nullptr; }
void pe(A *p) { delete p; }
void pf(A *p) { delete[] p; }
