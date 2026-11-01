// [expr.new]/24, [dcl.init.general]/7-9, [class.base.init]/7: a
// multidimensional array new of a class with a user-provided default
// constructor runs the constructor over every element; a mem-initializer
// `arr()` / `arr{}` and a declaration `K y[2][3]{}` of such an array
// call the constructor per element instead of zeroing; value-initializing
// array new of a class with an implicit constructor zeroes each element
// before constructing it.
struct K { K(); int v; };
K (*a())[3] { return new K[2][3]; }
K (*b(int n))[3] { return new K[n][3]; }
K (*c())[3] { return new K[2][3](); }
K (*d())[3] { return new K[2][3]{}; }
K* e() { return new K[2](); }
void f() { K x[2][3]; K y[2][3]{}; }
struct M { K arr[3]; M() : arr() {} };
struct N { K arr[3]; N() : arr{} {} };
struct M2 { K arr[2][3]; M2() : arr() {} };
void use() { M m; N n; M2 m2; }
struct Z { K k; int w; };
struct T { int a; virtual void f(); };
Z* z(int n) { return new Z[n](); }
Z (*z2())[2] { return new Z[3][2](); }
T* t() { return new T[2](); }
