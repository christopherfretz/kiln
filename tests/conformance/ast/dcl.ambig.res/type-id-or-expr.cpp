// [dcl.ambig.res]: a parenthesized type-id or expression is a type-id
// where both are possible; a parameter list beats a parenthesized
// declarator.
typedef int T;
int n;
unsigned long a = sizeof(T);
unsigned long b = sizeof(T(1));
unsigned long c = sizeof(int(*)());
unsigned long d = sizeof(int(*)[3]);
unsigned long e = sizeof(n);
unsigned long g = alignof(int(*)(T));
void h(int(T));           // parameter of type int(int), adjusted
void k(int(*p)(T, long)); // pointer-to-function parameter
void m(int(n));           // parameter named n
void q() {
  int (*fp)(int(T)) = nullptr;
  (void)fp;
}
