// _Generic (C11 6.5.1.1, a clang extension in C++; WP-1114): the
// GenericSelectionExpr prints the selected association's type and value
// category; its children are the controlling expression, then one
// `case 'T'` / `default` line per association holding its expression,
// the chosen one marked `selected` (WP-1131, spec/ast-dump.md §3).
//@ expect: ast
//@ oracle: clang
typedef long L;
int a = 1;
long b = 2;
int x = _Generic(a, int: 1, L: 2, default: 3);
int y = _Generic(b, int: 1, L: 2, default: 3);
int &r = _Generic(a, int: a, long: b);
int u = _Generic(a + 0.5, int: 1, default: 2);
// The type-operand form (clang's C2y extension) matches the type as
// written, qualifiers kept; it prints as `type_operand=`.
int t = _Generic(const int, int: 1, const int: 2);
// A qualified or sugared association type prints `'T':'desugared'`.
struct S {};
using CS = const S;
int q = _Generic(const L, const L: 1, volatile S: 3, CS: 4, default: 2);
