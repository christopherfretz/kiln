// [dcl.type.decltype], [dcl.ambig.res]/1: `decltype(m)::key_type` names a
// type, so `void g(decltype(m)::key_type);` declares a function, not a
// `void` variable initialized by an expression. The `(` of a declarator is
// a position where kiln classifies an unparsed `decltype(e)::name` by the
// tokens after it (tentative.rs `unparsed_decltype_scope`), and `)` `;`
// reads as an expression. Seen while reducing Bitcoin Core's
// `sizeof(decltype(m)::key_type)` (WP-811).
//@ expect: run
//@ stdout: 7
//@ xfail(kiln): WP-811 follow-up, an unparsed decltype(e)::name before ")" ";" in a declarator is classified as an expression
extern "C" int printf(const char*, ...);
struct M { using key_type = int; };
M m;
void g(decltype(m)::key_type);
void g(int k) { printf("%d\n", k); }
int main() { g(7); }
