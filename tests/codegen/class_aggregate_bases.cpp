// Aggregate initialization of classes with base subobjects (the list's
// first elements) and destructible members.
void note(const char*, int) noexcept;
struct D { int v; ~D() { note("~D", v); } };
struct A { int a; };
struct E {};
struct C : A, D, E { int c; D d; };
int f(int k) noexcept { C c{{k}, {k+1}, {}, 3, {4}}; C z{}; return c.c + z.a; }
