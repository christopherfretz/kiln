//@ expect: pp
// The worked examples of the C++ standard's [cpp.scope] and [cpp.replace]
// clauses: redefinition is only valid when identical, #undef ends a
// definition, expansion uses the definition in effect at the point of use,
// and rescanning interacts with argument pre-expansion.
#define x 3
#define f(a) f(x * (a))
#undef x
#define x 2
#define g f
#define z z[0]
#define h g(~
#define m(a) a(w)
#define w 0,1
#define t(a) a
#define p() int
#define q(x) x
#define r(x,y) x ## y
f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);
g(x+(3,4)-w) | h 5) & m
     (f)^m(m);
p() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };
#define str(s) # s
#define xstr(s) str(s)
#define INCFILE(n) vers ## n
xstr(INCFILE(2).hpp) str(strncmp("abc\0d", "abc", '\4') == 0)
#define OBJ_LIKE (1-1)
#define OBJ_LIKE /* white space */ (1-1) /* other */
#define FUNC_LIKE(a) ( a )
#define FUNC_LIKE( a )( /* note the white space */ \
                       a /* other stuff on this line
                         */ )
OBJ_LIKE FUNC_LIKE(z)
