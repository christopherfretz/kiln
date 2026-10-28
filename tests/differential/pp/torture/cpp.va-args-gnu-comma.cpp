//@ expect: pp
// The GNU `, ## __VA_ARGS__` extension: the comma is removed when the
// variadic argument is absent or empty, and kept otherwise. Compare with
// __VA_OPT__, the standard spelling.
#define E1(fmt, ...) call(fmt, ## __VA_ARGS__)
#define E2(fmt, ...) call(fmt __VA_OPT__(,) __VA_ARGS__)
#define E3(...) call(x, ## __VA_ARGS__)
E1("a") E1("a",) E1("a", 1) E1("a", 1, 2)
E2("a") E2("a",) E2("a", 1) E2("a", 1, 2)
E3() E3(1) E3(,)
#define NAMED(args...) call(args)
#define NAMED_COMMA(fmt, args...) call(fmt, ## args)
NAMED() NAMED(1, 2) NAMED_COMMA(f) NAMED_COMMA(f, 1)
