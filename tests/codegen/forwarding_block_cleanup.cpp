// [stmt.while], [stmt.do], [except.ctor]: clang folds a block that only
// branches into its successor (`SimplifyForwardingBlocks`) only while no
// cleanup is on the EH stack, so a loop inside a scope whose object has
// a destructor, or inside a `noexcept` function's terminate scope, keeps
// its forwarding condition block.
int g(int);
struct D { ~D(); };
void f() noexcept { while (1) { if (g(0)) break; } }
void h() { D d; while (1) { if (g(0)) break; } do { if (g(1)) break; } while (0); }
void k() { while (1) { if (g(0)) break; } }
