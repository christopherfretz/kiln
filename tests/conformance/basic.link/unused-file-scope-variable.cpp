// [basic.link], [dcl.attr.unused] (WP-217): unused file-scope variables
// (clang ShouldWarnIfUnusedFileScopedDecl, end of the translation unit): a
// namespace-scope variable with internal linkage (static, const, in an
// unnamed namespace) declared in the main file and never referenced is
// -Wunused-const-variable when const (also through a typedef or as an
// array element), else -Wunused-variable, in declaration order. Not an
// external, inline, [[maybe_unused]] or `used` variable, not a referenced
// one (also from a template), and not one whose initialization or
// destruction has side effects.
//@ args: -Wunused-variable
//@ strict-diags
//@ diag: 37:12 warning "unused variable 's1'"
//@ diag: 38:12 warning "unused variable 's2'"
//@ diag: 40:18 warning "unused variable 'c1'"
//@ diag: 41:11 warning "unused variable 'c2'"
//@ diag: 42:15 warning "unused variable 'c3'"
//@ diag: 44:10 warning "unused variable 't1'"
//@ diag: 50:5 warning "unused variable 'n1'"
//@ diag: 51:19 warning "unused variable 'n2'"
//@ diag: 54:12 warning "unused variable 'ns1'"
//@ diag: 56:12 warning "unused variable 'arr'"
//@ diag: 63:12 warning "unused variable 's9'"
//@ diag: 63:20 warning "unused variable 's10'"
//@ diag: 65:10 warning "unused variable 't2'"
//@ diag: 66:25 warning "unused variable 'tl'"
//@ diag: 67:18 warning "unused variable 'ca'"
//@ diag: 69:11 warning "unused variable 'ci'"
//@ diag: 71:1 warning "unused variable 'mv'"
//@ diag: 70:22 note "expanded from macro 'V'"
int f();
struct D {
  ~D();
};
struct T {
  int a;
};
static int s1;
static int s2 = 3;
static int s3 = f();
static const int c1 = 1;
const int c2 = 2;
constexpr int c3 = 3;
static D d1;
static T t1;
static const int c4 = 4;
int use4[c4];
static int s4;
int g() { return s4; }
namespace {
int n1;
const char *const n2 = "x";
}
namespace N {
static int ns1;
}
static int arr[3];
inline int iv = 1;
static int s6 [[maybe_unused]];
static int s7 __attribute__((used));
extern const int e1 = 1;
static constexpr int c5 = 5;
template <class X> int tf() { return c5; }
static int s9 = 1, s10;
const int c6 = f();
static T t2 = {1};
static thread_local int tl;
static const int ca[2] = {};
typedef const int CI;
static CI ci = 1;
#define V static int mv;
V
int ext;
