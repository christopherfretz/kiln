// clang's lifetime attributes (WP-674): [[clang::lifetimebound]] applies to
// parameters and, after a function declarator, to the implicit object
// parameter of a non-static member function; it is an error on a type,
// a declaration, a static member or non-member function's implicit
// object, a constructor and a destructor. [[gsl::Owner]] and
// [[gsl::Pointer]] apply to structs and classes (elsewhere they are
// ignored with a warning) and take an optional type argument that is
// neither a reference nor an array; both kinds on one class, or differing
// arguments on its redeclarations, are not compatible.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ requires: clang<22
//@ diag: 42:27 error "'lifetimebound' attribute only applies to parameters and implicit object parameters"
//@ diag: 43:21 error "'lifetimebound' attribute cannot be applied; static member function has no implicit object parameter"
//@ diag: 44:9 error "'lifetimebound' attribute cannot be applied to a constructor"
//@ diag: 45:10 error "'lifetimebound' attribute cannot be applied to a destructor"
//@ diag: 48:5 error "'lifetimebound' attribute only applies to parameters and implicit object parameters"
//@ diag: 51:12 error "'lifetimebound' attribute cannot be applied; non-member function has no implicit object parameter"
//@ diag: 52:3 error "'lifetimebound' attribute only applies to parameters and implicit object parameters"
//@ diag: 54:18 error "'lifetimebound' attribute cannot be applied; non-member function has no implicit object parameter"
//@ diag: 55:7 error "'lifetimebound' attribute cannot be applied to types"
//@ diag: 59:15 error "a reference type is an invalid argument to attribute 'Pointer'"
//@ diag: 60:15 error "an array type is an invalid argument to attribute 'Pointer'"
//@ diag: 61:22 error "'Pointer' and 'Owner' attributes are not compatible"
//@ diag: 61:10 note "conflicting attribute is here"
//@ diag: 62:9 warning "'Owner' attribute only applies to structs"
//@ diag: 63:8 warning "'Owner' attribute only applies to structs"
//@ diag: 64:3 warning "'Owner' attribute only applies to structs"
//@ diag: 65:24 error "expected ')'"
//@ diag: 65:29 error "expected ','"
//@ diag: 65:26 warning "unknown attribute 'int' ignored"
//@ diag: 66:21 error "expected a type"
//@ diag: 68:15 error "'Owner' and 'Owner' attributes are not compatible"
//@ diag: 67:15 note "conflicting attribute is here"
//@ diag: 69:10 error "'Pointer' and 'Owner' attributes are not compatible"
//@ diag: 67:15 note "conflicting attribute is here"
//@ diag: 72:10 warning "unknown attribute 'owner' ignored"
struct S {
  int x;
  int& a() [[clang::lifetimebound]];
  int& b() __attribute__((lifetimebound));
  static int& c() [[clang::lifetimebound]];
  S() [[clang::lifetimebound]];
  ~S() [[clang::lifetimebound]];
  int& d(int& p [[clang::lifetimebound]]);
  int& e(int& p __attribute__((lifetimebound)));
  [[clang::lifetimebound]] int y;
  int& f() const & noexcept [[clang::lifetimebound]];
};
int& g() [[clang::lifetimebound]];
[[clang::lifetimebound]] int& h(int& x);
int& i(int& [[clang::lifetimebound]] x);
int& j(int& x) [[clang::lifetimebound(1)]];
int [[clang::lifetimebound]] k;
void v(int& x [[clang::lifetimebound]]);
struct [[gsl::Owner]] O1 {};
struct [[gsl::Owner(int)]] O2 {};
struct [[gsl::Pointer(int&)]] P1 {};
struct [[gsl::Pointer(int[2])]] P2 {};
struct [[gsl::Owner, gsl::Pointer]] OP {};
union [[gsl::Owner]] U1 {};
enum [[gsl::Owner]] E1 {};
[[gsl::Owner]] int vv;
struct [[gsl::Owner(int, int)]] O3 {};
struct [[gsl::Owner(1)]] O4 {};
struct [[gsl::Owner(int)]] O5;
struct [[gsl::Owner(float)]] O5 {};
struct [[gsl::Pointer]] O5;
class [[gsl::Pointer]] C1;
template<class T> struct [[gsl::Owner(T)]] TO {};
struct [[gsl::owner]] lower {};
struct [[gsl::Owner]] [[gsl::Owner]] Twice {};
