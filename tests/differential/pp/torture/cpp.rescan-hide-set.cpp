//@ expect: pp
// [cpp.rescan]: a macro name is not replaced again while its own expansion
// is being rescanned (the "hide set"), but a name that only appears after
// the expansion ended is fair game.

// Direct and indirect self-reference stop after one round.
#define self self + 1
#define ping pong
#define pong ping
self; ping; pong;

// The [cpp.rescan] example: f(2)(9) rescans `2*g` followed by `(9)` from
// the source; g is not inside f's expansion any more when it takes `(9)`.
#define f(a) a*g
#define g(a) f(a)
f(2)(9);

// A function-like name produced by an expansion picks up arguments from
// the tokens after it; it is still painted if the expansion that painted it
// covers those tokens.
#define NIL(x) x
#define G0(arg) NIL(G1)(arg)
#define G1(arg) NIL(arg)
G0(42);

// Painted tokens stay painted: a name blocked inside an argument is not
// expanded later, even when the argument is substituted elsewhere.
#define ID(x) x
#define REC REC_
#define REC_ ID(REC)
REC;
ID(ID(ID(ID(1))));

// Arguments are fully expanded before substitution, so a name that is
// blocked only inside the argument's expansion still blocks.
#define LOOP(x) x LOOP
LOOP(LOOP(0))(1)(2);
