// offsetof with an array index that is not a constant lowers as clang's
// ScalarExprEmitter::VisitOffsetOfExpr: from 0, each component added in
// turn with a plain `add` (constants folded as IRBuilder folds them), an
// index cast to size_t by its own signedness and multiplied by its
// element size with a plain `mul`; a constant designator is one
// constant, a negative constant index wraps. No bounds check under
// -fsanitize=undefined (clang emits none; the bool and enum load checks
// are off because kiln checks enum loads in C, which clang does not).
//@ expect: ir
//@ args: -std=gnu17 -fsanitize=undefined -fno-sanitize=function,bool,enum
#include <stddef.h>
struct in { char c; int x[3][5]; };
struct out { int n; struct in a[4]; long data[]; };
enum e { E1 = 1 };
size_t f1(size_t count) { return offsetof(struct out, data[count]); }
size_t f2(int i, int j) { return offsetof(struct out, a[i].x[j][2]); }
size_t f3(short s) { return offsetof(struct out, a[s]); }
size_t f4(unsigned char c) { return offsetof(struct out, a[1].x[c]); }
size_t f5(__int128 w) { return offsetof(struct out, data[w]); }
size_t f6(unsigned u) { return offsetof(struct out, data[u]); }
size_t f7(int *p) { return offsetof(struct out, data[(*p)++]); }
size_t f8(enum e v, _Bool b) { return offsetof(struct out, a[v].x[b][b]); }
size_t f9(void) { return offsetof(struct out, a[-1]); }
size_t f10(void) { return offsetof(struct out, a[3].x[2][4]); }
size_t f11(int i) { return offsetof(struct out, a[i]) + offsetof(struct out, data[-100]); }
