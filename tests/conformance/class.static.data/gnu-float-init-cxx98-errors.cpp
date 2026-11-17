// [class.static.data]/4 at C++98: the GNU extension for a floating or complex
// floating `static const` member needs an initializer that folds to a
// constant; other non-integral types (`_Complex int`, a class) must be
// initialized out of line; a template member is checked when instantiated.
// Verified against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++98
//@ diag: 19:23 warning "in-class initializer for static data member of type 'const double' is a GNU extension"
//@ diag: 19:27 error "in-class initializer for static data member is not a constant expression"
//@ diag: 20:22 warning "in-class initializer for static data member of type 'const float' is a GNU extension"
//@ diag: 20:26 error "in-class initializer for static data member is not a constant expression"
//@ diag: 21:29 error "static data member of type 'const _Complex int' must be initialized out of line"
//@ diag: 23:45 warning "in-class initializer for static data member of type 'const double' is a GNU extension"
//@ diag: 24:11 note "in instantiation of template class 'B<double>' requested here"
double g();
int n;
struct A {
  static const double d = g();
  static const float f = n;
  static const _Complex int ci = 1;
};
template<class T> struct B { static const T t = 1.5; };
B<double> b;
