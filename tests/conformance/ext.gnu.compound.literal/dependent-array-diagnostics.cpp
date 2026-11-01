// Dependent array compound literals ([GNU]; [temp.dep.type]): the
// literal keeps its list as written and is built again, then checked,
// for each instantiation (clang's TransformCompoundLiteralExpr).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:50 error "excess elements in array initializer"
//@ diag: 17:33 note "in instantiation of function template specialization 'g<2>' requested here"
//@ diag: 14:37 error "function-style cast to a builtin type can only take one argument"
//@ diag: 17:51 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 16:44 error "cannot initialize an array element of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 17:62 note "in instantiation of function template specialization 'z<int>' requested here"
template<class T> int f() { return ((T[]){1, 2, 3, 4})[1]; }
template<int N> int g() { return ((int[N]){1, 2, 3})[0]; }
template<class T> int h() { return ((T){1, 2}).a; }
struct S { int a, b; };
template<class T> int z() { return ((T[2]){"a"})[0]; }
int main() { return f<char>() + g<2>() + h<S>() + h<int>() + z<int>(); }
