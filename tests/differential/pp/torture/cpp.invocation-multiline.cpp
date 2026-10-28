//@ expect: pp
// Function-like macro invocations may span lines, nest, and have their `(`
// on a later line; the output puts the expansion where the name was.
#define f(x) [x]
#define g(x, y) f(x) f(y)
g(
  f(1),
  f(
    2
  )
) end1;
f
(3) end2;
f

   (4) end3;
f /* comment */ ( /* comment */ 5 /* comment */ ) end4;
f // line comment
(6) end5;
g(f
  (7), f
  ) end6;
// Not followed by `(`: not an invocation, the name is output as is.
f;
f
;
int f = 0;
