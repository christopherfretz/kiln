// [class.qual]/2: `C::C` names the constructors of C, not the class, where
// the name is not a class-name: a functional cast, a declaration's type
// specifier or a sizeof operand is an error.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:12 error "qualified reference to 'S' is a constructor name rather than a type in this context"
//@ diag: 16:6 error "qualified reference to 'S' is a constructor name rather than a type in this context"
//@ diag: 17:6 error "qualified reference to 'S' is a constructor name rather than a type in this context"
//@ diag: 18:19 error "qualified reference to 'S' is a constructor name rather than a type in this context"
//@ diag: 19:11 error "qualified reference to 'X' is a constructor name rather than a type in this context"
struct S { S(int); typedef int I; };
template<class T> struct X { X(int); };
void f() {
  (void)S::S(1);
  S::S *p;
  S::S x(3);
  (void)sizeof(S::S);
  X<int>::X y(4);
}
