// [expr.new]/6 (C++98): the array bound of a new-expression has integral
// or enumeration type; a class type converted by its conversion function
// is a C++11 extension (clang `ext_array_size_conversion`, -pedantic),
// diagnosed at the new-expression.
//@ expect: compile-ok
//@ std: c++98
//@ args: -pedantic
//@ strict-diags
//@ diag: 14:12 warning "implicit conversion from array size expression of type 'N' to integral type 'int' is a C++11 extension"
//@ diag: 16:17 warning "implicit conversion from array size expression of type 'N' to integral type 'int' is a C++11 extension"
//@ diag: 21:25 warning "implicit conversion from array size expression of type 'M' to enumeration type 'En' is a C++11 extension"
struct N { operator int() const { return 3; } };
void f(N n) {
  int *p = new int[n];
  delete[] p;
  int (*q)[2] = new int[n][2];
  delete[] q;
}
enum En { k3 = 3 };
struct M { operator En() const { return k3; } };
void g(M m) { char *p = new char[m]; delete[] p; }
