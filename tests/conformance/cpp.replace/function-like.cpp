// [cpp.replace], [cpp.stringize], [cpp.concat], [cpp.rescan], __VA_OPT__.
//@ expect: pp
#define STR(x) #x
#define XSTR(x) STR(x)
#define CAT(a, b) a ## b
#define TWICE(f, x) f(f(x))
#define INC(x) ((x) + 1)
#define CALL(fn, ...) fn(0 __VA_OPT__(,) __VA_ARGS__)
STR(  hello    "world"  \n )
XSTR(CAT(1, 2))
CAT(foo, bar) CAT(, x) CAT(y, )
TWICE(INC, 0)
CALL(g)
CALL(g, 1, 2)
// [cpp.rescan] leaves this nesting unspecified; kiln matches clang.
#define f(x) x g
#define g f
f(1)(2)(3)
