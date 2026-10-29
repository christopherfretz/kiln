// [stmt.ambig]: a statement that could be an expression-statement or a
// declaration is a declaration; otherwise it is an expression.
typedef int T;
int x;
int f(int);
void g() {
  T(a);             // declaration of a
  T(*b);            // declaration of pointer b
  T(c) = 1;         // declaration with initializer
  T(f)(1);          // expression: call through a functional cast
  T(d)[3];          // declaration of an array
  T(*h)(int);       // declaration of a pointer to function
  (void)T(x);       // expression
  T(y), z(2);       // declaration of y and z
  (void)a; (void)b; (void)c; (void)d; (void)h; (void)y; (void)z;
}
