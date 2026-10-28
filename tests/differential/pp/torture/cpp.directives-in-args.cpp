//@ expect: pp
// Directives inside a macro's argument list are undefined behavior per
// [cpp.replace.general]/13; clang (like GCC) processes them in place, and
// real code (`assert(` with `#ifdef` in it) depends on that.
#define ID(x) x
#define PAIR(a, b) [a][b]
ID(
#ifdef UNDEFINED_THING
  wrong
#else
  right
#endif
) end1;
PAIR(1,
#if 1
  2
#endif
) end2;
#define X old
ID(X
#undef X
#define X new
X) end3;
X end4;
// A directive between the name and `(`: the name is not an invocation.
ID
#define BETWEEN 1
(BETWEEN) end5;
