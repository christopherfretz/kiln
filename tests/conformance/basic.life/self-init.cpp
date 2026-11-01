// [basic.life]/7: a variable used in its own initializer before its
// lifetime begins (clang `CheckSelfReference`): non-local variables, and
// locals of class or reference type (other locals are clang's CFG
// analysis); `T a = a;` for a non-class `T`, unevaluated operands, static
// and extern "C" variables of non-class type, earlier members in an
// initializer list and `constexpr` variables are not diagnosed.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 34:9 warning "variable 'x' is uninitialized when used within its own initialization"
//@ diag: 36:8 warning "variable 'y2' is uninitialized when used within its own initialization"
//@ diag: 37:8 warning "variable 'y3' is uninitialized when used within its own initialization"
//@ diag: 38:72 warning "variable 's' is uninitialized when used within its own initialization"
//@ diag: 40:10 warning "variable 's3' is uninitialized when used within its own initialization"
//@ diag: 41:8 warning "variable 's4' is uninitialized when used within its own initialization"
//@ diag: 43:14 warning "variable 's5' is uninitialized when used within its own initialization"
//@ diag: 44:10 warning "reference 'r' is not yet bound to a value when used within its own initialization"
//@ diag: 46:16 warning "variable 't' is uninitialized when used within its own initialization"
//@ diag: 47:23 warning "variable 'n' is uninitialized when used within its own initialization"
//@ diag: 47:41 warning "variable 'z' is uninitialized when used within its own initialization"
//@ diag: 47:53 warning "variable 'qn' is uninitialized when used within its own initialization"
//@ diag: 48:9 warning "variable 'u' is uninitialized when used within its own initialization"
//@ diag: 52:16 warning "variable 'r' is uninitialized when used within its own initialization"
//@ diag: 53:18 warning "variable 'mv' is uninitialized when used within its own initialization"
//@ diag: 54:17 warning "variable 'il' is uninitialized when used within its own initialization"
//@ diag: 59:18 warning "static variable 'w' is suspiciously used within its own initialization"
//@ diag: 61:10 warning "variable 'ls' is uninitialized when used within its own initialization"
//@ diag: 62:13 warning "reference 'lr' is not yet bound to a value when used within its own initialization"
//@ diag: 65:27 warning "variable 'tv<int>' is uninitialized when used within its own initialization"
//@ diag: 66:10 note "in instantiation of variable template specialization 'tv<int>' requested here"
//@ diag: 68:14 warning "variable 't2' is uninitialized when used within its own initialization"
//@ diag: 68:19 warning "variable 't2' is uninitialized when used within its own initialization"
//@ diag: 69:11 warning "variable 't3' is uninitialized when used within its own initialization"
namespace std { template <class T> T &&move(T &t) noexcept { return static_cast<T &&>(t); } }
int x = x + 1;
int y = y;
int y2(y2);
int y3{y3};
struct S { int a, b; int f() const; static int st; int arr[2]; } s = { s.a };
S s2 = { 1, s2.a };
S s3 = { s3.b, 2 };
S s4 = s4;
int m = s4.f() + (s4.st);
S s5 = { int(s5.f()) };
int &r = r;
int q = sizeof(q) + (q = 1) + noexcept(q);
int t = true ? t : 1;
namespace N { int n = n * 2; long z = ++z; int qn = N::qn + 1; }
int u = u += 1;
const int *pa = &s.a;
extern "C" int ec = ec + 1;
struct R { int a; static const R r; R(); R(const R &); };
const R R::r = R::r;
R mv = std::move(mv);
inline int il = il + 1;
struct T { static const int k; static int l; };
const int T::k = T::k + 1;
int T::l = l + 1;
void f() {
  static int w = w + 1;
  static int w2 = w2;
  S ls = ls;
  int &lr = lr;
  int li = li + 1;
}
template <class U> U tv = tv<U> + 1;
int ti = tv<int>;
// Both arms of a glvalue conditional are value uses (clang HandleValue).
int t2 = x ? t2 : t2;
int t3 = (t3 ? x : y);
