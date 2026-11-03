// GNU `init_priority(N)` ([dcl.attr.grammar]/5, implementation-defined;
// clang handleInitPriorityAttr, WP-189): exactly one argument, on a
// variable, not at block scope, of class type or an array of it; the
// argument is an integer constant that fits in 32 bits. The range check
// (which clang 19 and 22 word differently) is in
// gnu-init-priority-range{,-clang19}.cpp. A static data member is a
// namespace-scope object ([basic.start.dynamic]/1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:38 error "can only use 'init_priority' attribute on file-scope definitions of objects of class type"
//@ diag: 23:20 error "'init_priority' attribute takes one argument"
//@ diag: 24:21 error "'init_priority' attribute takes one argument"
//@ diag: 25:22 error "can only use 'init_priority' attribute on file-scope definitions of objects of class type"
//@ diag: 26:21 error "can only use 'init_priority' attribute on file-scope definitions of objects of class type"
//@ diag: 27:40 error "can only use 'init_priority' attribute on file-scope definitions of objects of class type"
//@ diag: 28:34 error "integer constant expression evaluates to value 4294967296 that cannot be represented in a 32-bit unsigned integer type"
//@ diag: 30:20 error "'init_priority' attribute requires an integer constant"
//@ diag: 31:20 error "'init_priority' attribute requires an integer constant"
//@ diag: 32:25 error "'init_priority' attribute only applies to variables"
//@ diag: 33:23 error "'init_priority' attribute only applies to variables"
struct S { S(); };
void f() { static S l __attribute__((init_priority(200))); }
S a __attribute__((init_priority)) ;
S a2 __attribute__((init_priority(1, 2)));
int g __attribute__((init_priority(200)));
S *p __attribute__((init_priority(300)));
template <class T> T vt __attribute__((init_priority(300)));
S b __attribute__((init_priority(4294967296)));
int n = 5;
S c __attribute__((init_priority(n)));
S k __attribute__((init_priority(1.5)));
void h() __attribute__((init_priority(200)));
struct __attribute__((init_priority(200))) T {};
struct C { static S m __attribute__((init_priority(300))); };
S C::m;
S arr[2] __attribute__((init_priority(400)));
constexpr int P = 400;
S z __attribute__((init_priority(P + 1)));
namespace ns { S q __attribute__((init_priority(65535))); }
