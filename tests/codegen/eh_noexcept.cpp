// [except.spec]/5, [except.terminate]: a noexcept function's
// terminate landing pad; locals destroyed on unwind; fall-off traps.
struct S { S(); ~S(); };
void f();
void g() noexcept;
void a() { S s; f(); g(); }
void b() noexcept { f(); }
int h(int x) { if (x) return 1; }
int k(int x) { try { f(); } catch (...) {} }
