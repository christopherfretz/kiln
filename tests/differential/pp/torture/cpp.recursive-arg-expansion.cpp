//@ expect: pp
// Arguments are fully macro-expanded before substitution, except where
// they are operands of # or ##. Nested invocations of the same macro in an
// argument are expanded; the outer name painted blue during rescanning
// stays unexpanded even when it later appears with `(`.
#define F(x) f(x)
#define G(x) F(x) #x
#define ID(x) x
#define TWICE(x) x x
F(F(F(1)))
G(F(2))
ID(ID(ID(ID(3))))
TWICE(TWICE(TWICE(4)))
#define BLUE BLUE_F(
#define BLUE_F(x) x BLUE
BLUE 5) 6)
#define REC(x) x REC
REC(1)(2)
#define AGAIN(x) x
#define AGAIN_NAME AGAIN
AGAIN(AGAIN_NAME)(7)
