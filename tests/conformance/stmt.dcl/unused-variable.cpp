// [stmt.dcl], [dcl.attr.unused] (WP-203): -Wunused-variable (clang
// ShouldDiagnoseUnusedDecl): an unreferenced local variable is diagnosed
// as its scope is left, sorted by location and not once an error occurred
// in the scope; not one marked [[maybe_unused]] / unused (also through
// its typedef or class), a `_` placeholder, an extern or exception
// variable, nor one whose class has a non-trivial destructor or a
// non-trivial constructor that is not a constant initialization. A
// variable of dependent type is diagnosed as it is instantiated.
//@ expect: compile-fail
//@ args: -Wunused-variable
//@ strict-diags
//@ diag: 52:14 warning "equality comparison result unused"
//@ diag: 52:14 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 77:21 warning "unused variable 'inner'"
//@ diag: 78:9 warning "unused variable 'nested'"
//@ diag: 79:12 warning "unused variable 'i'"
//@ diag: 52:7 warning "unused variable 'y'"
//@ diag: 53:7 warning "unused variable 'a'"
//@ diag: 53:10 warning "unused variable 'b'"
//@ diag: 60:14 warning "unused variable 'st'"
//@ diag: 61:13 warning "unused variable 'ci'"
//@ diag: 62:17 warning "unused variable 'cx'"
//@ diag: 63:8 warning "unused variable 'ref'"
//@ diag: 64:14 warning "unused variable 'lt'"
//@ diag: 66:8 warning "unused variable 't'"
//@ diag: 67:8 warning "unused variable 't2'"
//@ diag: 70:9 warning "unused variable 'cc'"
//@ diag: 72:8 warning "unused variable 'arr'"
//@ diag: 74:8 warning "unused variable 'pi'"
//@ diag: 77:8 warning "unused variable 'l'"
//@ diag: 56:15 warning "expression result unused"
//@ diag: 84:35 warning "unused variable 'z'"
//@ diag: 85:46 warning "unused variable 'q'"
//@ diag: 87:20 warning "unused variable 'lam'"
//@ diag: 88:22 error "use of undeclared identifier 'undeclared'"
//@ diag: 89:21 warning "unused variable 'e2'"
//@ diag: 89:37 error "use of undeclared identifier 'undeclared2'"
//@ diag: 84:40 warning "unused variable 'w'"
//@ diag: 86:15 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 84:45 warning "unused variable 'w2'"
//@ diag: 85:51 warning "unused variable 'k'"
//@ diag: 86:48 note "in instantiation of member function 'TS<int>::m' requested here"
struct Triv { int a; };
struct Dtor { ~Dtor(); };
struct Ctor { Ctor(); };
struct CCtor { constexpr CCtor() {} int a = 0; };
struct [[maybe_unused]] Tagged { };
typedef int unused_int __attribute__((unused));
struct Inc;
int g;
int f(int x) {
  int y = (x == 1, 2);
  int a, b = 1, c;
  c = 3;
  int used = 4; (void)used;
  int r1 = 1; sizeof(r1);
  [[maybe_unused]] int mu;
  int __attribute__((unused)) au;
  unused_int ui;
  static int st = 5;
  const int ci = 6;
  constexpr int cx = 7;
  int &ref = g;
  const int &lt = 8;
  const Dtor &ltd = Dtor();
  Triv t;
  Triv t2{1};
  Dtor d;
  Ctor ct;
  CCtor cc;
  Tagged tg;
  Triv arr[3];
  Dtor darr[2];
  Inc *pi;
  extern int ex;
  int _ = 1;
  auto l = [] { int inner; };
  { int nested; }
  for (int i = 0;;) { break; }
  if (int cond = x) {}
  try {} catch (int e) {}
  return x;
}
template<class T> void tf() { int z; T w; T w2 = T(); }
template<class T> struct TS { void m() { int q; T k; } };
void inst() { tf<int>(); tf<Dtor>(); TS<int>().m(); }
auto gl = [] { int lam; return 0; };
void err() { int e1; undeclared; }
void err2() { { int e2; } { int e3; undeclared2; } }
void f2() { [[maybe_unused]] int m1; [[gnu::unused]] int m2; int m3 [[maybe_unused]]; }
