// WP-432: __builtin_align_up / __builtin_align_down / __builtin_is_aligned
// on a pointer to an object whose alignment is known fold in a global
// initializer (clang `PointerExprEvaluator`); an unknown result (the
// object's alignment is less than the requested one) is initialized
// dynamically. Same at clang 19 and 22.

alignas(16) char buf[20];
char *g1 = __builtin_align_up(&buf[3], 8);
char *g0 = __builtin_align_down(&buf[19], 16);
char *g5 = __builtin_align_up(&buf[16], 16);
bool g2 = __builtin_is_aligned(&buf[3], 32);
bool g3 = __builtin_is_aligned(&buf[4], 4);
bool g6 = __builtin_is_aligned(&buf[2], 4);
int x;
int *g4 = __builtin_align_up(&x, 8);
