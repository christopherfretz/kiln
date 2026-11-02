// [dcl.align]/2-5: an alignment-specifier on a variable sets the alignment
// of its storage (globals, extern declarations, static locals, allocas,
// catch variables) and the value of alignof applied to it (clang
// `getDeclAlign`, `GetAlignOfExpr`). A dependent alignas on a local of a
// function template is resolved at instantiation; x86-64 raises a global
// array of 16 bytes or more to 16 for storage only.
//@ args: -Wno-gnu-alignof-expression
alignas(8) char g;
char g3 alignas(16);
alignas(32) extern char ge;
alignas(4) int gi = 7;
int use() { alignas(16) char l = 1; char l2 alignas(64) = 2; alignas(8) static char ls; return l + l2 + ls + ge + g3; }
struct S { char c; alignas(8) char m; char n; };
S sx;
int f() { return __alignof__(g) + alignof(sx.m) + alignof(sx.n) + alignof(gi); }
template<int N> int tf() { alignas(N) char x = 0; return alignof(x) + x; }
int u2 = tf<32>();
char arr[20];
int fa() { return alignof(arr) + arr[0]; }
void thrower();
int c() { try { thrower(); } catch (int e) { alignas(16) int k = e; return k; } return 0; }
