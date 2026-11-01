// [class.union]/2, [dcl.init.aggr]/5 (WP-213): a union global initialized
// through a member other than the first (designated, constexpr-evaluated,
// nested in a struct, in an array, static local, referenced before its
// definition) holds that member's value at run time.
//@ expect: run
//@ stdout: 1 1.5 2.5 9 5 4 2 3 3
//@ strict-diags
extern "C" int printf(const char*, ...);
union U { int i; double d; };
struct S { int a; union { char c; long long l; }; };
struct T { U u[3]; short k; };
constexpr U mku() { U x{}; x.d = 2.5; return x; }
constexpr S mks() { S s{}; s.l = 9; return s; }
constexpr T mkt() { T t{}; t.u[0].i = 1; t.u[1].d = 2.5; t.k = 4; return t; }
extern U u0;
extern U ud;
int read_early() { return u0.i; }
double read_d() { return ud.d; }
U u0 = {.i = 1};
U ud = {.d = 1.5};
U u2 = mku();
S s1 = mks();
S s2 = {1, {.l = 5}};
T t1 = mkt();
U ua[2] = {{.d = 1.0}, {.i = 2}};
int f() { static U su = {.i = 3}; return su.i; }
struct H { static U su; };
U H::su = {.i = 3};
int main() {
  printf("%d %g %g %lld %lld %d %d %d %d\n", read_early(), read_d(), u2.d, s1.l, s2.l, t1.k,
         ua[1].i, f(), H::su.i);
  return t1.u[0].i == 1 && t1.u[1].d == 2.5 ? 0 : 1;
}
