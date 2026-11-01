// [class.union]/2, [dcl.init.aggr]/5 (WP-213): a union global initialized
// through a member other than the one its IR type is built from gets the
// IR type of its initializer (the active member, `undef`-padded to the
// union's size). A declaration created by an earlier use has the union's
// type and is replaced: the definition is appended at the end of the
// globals, as clang's `GetOrCreateLLVMGlobal` does. Covers designated and
// constexpr-evaluated initializers (the active member comes from the
// Oven's image), unions nested in structs, arrays of unions, static data
// members and static locals.
union U { int i; double d; };
struct S { int a; union { char c; long long l; }; };
struct T { U u[3]; short k; };
union N { char c; struct { short a; union { int x; float y; } q; } p; };
constexpr U mku() { U x{}; x.d = 2.0; return x; }
constexpr S mks() { S s{}; s.l = 9; return s; }
constexpr T mkt() { T t{}; t.u[0].i = 1; t.u[1].d = 2.5; t.k = 4; return t; }
extern U u0; extern S s1; extern T t1; extern U ua[2];
int use() { return u0.i + s1.a + t1.k + ua[0].i; }
int mid = 1;
U u0 = {.i = 1};
U ud = {.d = 1.0};
U u2 = mku();
S s1 = mks();
S s2 = {1, {.l = 5}};
T t1 = mkt();
U ua[2] = {{.d = 1.0}, {.i = 2}};
N n1 = {.p = {1, {.y = 2.0f}}};
constexpr U cu = {.i = 7};
const U* pcu = &cu;
int f() {
  static U su = {.i = 3};
  static S ls = mks();
  static T lt = mkt();
  static N ln = {.p = {1, {2}}};
  return su.i + ls.a + lt.k + ln.c;
}
struct H { static U su; static inline U siu = {.i = 4}; };
U H::su = {.i = 3};
int g() { return H::su.i + H::siu.i; }
int z = 3;
