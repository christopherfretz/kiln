// [dcl.type.decltype], [dcl.ambig.res]/1: `decltype(m)::key_type` names a
// type, so `void g(decltype(m)::key_type);` declares a function, not a
// `void` variable initialized by an expression. The `(` of a declarator is
// a position where kiln does not parse `decltype(e)` ahead; for a variable
// operand it looks `key_type` up in the variable's class instead of
// guessing from the tokens after it (WP-843, fixed by WP-851). Seen while
// reducing Bitcoin Core's `sizeof(decltype(m)::key_type)` (WP-811).
//@ expect: run
//@ stdout: 7
extern "C" int printf(const char*, ...);
struct M { using key_type = int; };
M m;
void g(decltype(m)::key_type);
void g(int k) { printf("%d\n", k); }
int main() { g(7); }
