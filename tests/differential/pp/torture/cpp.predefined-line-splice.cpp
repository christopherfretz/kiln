//@ expect: pp
// __LINE__ is the presumed line of the __LINE__ token itself: line splices
// move it, and inside a multi-line macro invocation clang uses the line
// where the invocation's `)` is.
int a = __LINE__;
int b = __LI\
NE__;
int c = \
\
__LINE__;
#define ID(x) x
#define LINE_HERE __LINE__
int d = ID(
  __LINE__
);
int e = ID(LINE_HERE
);
int f = LINE_HERE;
#define TWO(x, y) x y
int g[] = { TWO(__LINE__,
                __LINE__) };
int h = __LINE__ /* a
multi-line comment */ + __LINE__;
