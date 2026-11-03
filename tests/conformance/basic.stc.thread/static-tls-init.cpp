// [basic.stc.thread], [basic.start.static]/2 (GNU C++98 edits for
// `__thread`): a `__thread` or `_Thread_local` variable has static TLS
// (clang `TLS_Static`). Its initializer must be a constant initializer
// (clang `Expr::isConstantInitializer`: aggregate lists, trivial
// constructions, parentheses and value-preserving casts are looked
// through, a reference binds a constant address, anything else must
// fold; the error is at the first part that does not) and its type must
// be trivially destructible. `thread_local` allows both; an `extern`
// declaration is not checked. Same in clang 22 and 19, except that clang 19
// reports `int d8{f()}` at the `{` (kiln at -fclang-compat=19 too).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 78:19 error "initializer for thread-local variable must be a constant expression"
//@ diag: 78:14 note "use 'thread_local' to allow this"
//@ diag: 79:12 error "initializer for thread-local variable must be a constant expression"
//@ diag: 79:12 note "use 'thread_local' to allow this"
//@ diag: 80:12 error "initializer for thread-local variable must be a constant expression"
//@ diag: 80:12 note "use 'thread_local' to allow this"
//@ diag: 81:17 error "initializer for thread-local variable must be a constant expression"
//@ diag: 81:17 note "use 'thread_local' to allow this"
//@ diag: 82:19 error "initializer for thread-local variable must be a constant expression"
//@ diag: 82:14 note "use 'thread_local' to allow this"
//@ diag: 83:20 error "initializer for thread-local variable must be a constant expression"
//@ diag: 83:15 note "use 'thread_local' to allow this"
//@ diag: 84:20 error "initializer for thread-local variable must be a constant expression"
//@ diag: 84:15 note "use 'thread_local' to allow this"
//@ diag: 85 error "initializer for thread-local variable must be a constant expression"
//@ diag: 85:14 note "use 'thread_local' to allow this"
//@ diag: 86:21 error "initializer for thread-local variable must be a constant expression"
//@ diag: 86:12 note "use 'thread_local' to allow this"
//@ diag: 87:27 error "initializer for thread-local variable must be a constant expression"
//@ diag: 87:14 note "use 'thread_local' to allow this"
//@ diag: 88:19 error "initializer for thread-local variable must be a constant expression"
//@ diag: 88:12 note "use 'thread_local' to allow this"
//@ diag: 91:12 error "type of thread-local variable has non-trivial destruction"
//@ diag: 91:12 note "use 'thread_local' to allow this"
//@ diag: 92:13 error "type of thread-local variable has non-trivial destruction"
//@ diag: 92:13 note "use 'thread_local' to allow this"
//@ diag: 93:12 error "type of thread-local variable has non-trivial destruction"
//@ diag: 93:12 note "use 'thread_local' to allow this"
//@ diag: 102:21 error "initializer for thread-local variable must be a constant expression"
//@ diag: 102:21 note "use 'thread_local' to allow this"
//@ diag: 103:28 error "initializer for thread-local variable must be a constant expression"
//@ diag: 103:23 note "use 'thread_local' to allow this"
//@ diag: 104:21 error "type of thread-local variable has non-trivial destruction"
//@ diag: 104:21 note "use 'thread_local' to allow this"
int f();
struct C { constexpr C(int v) : v(v) {} int v; };
struct N { N(int v) : v(v) {} N() : v(0) {} int v; };
struct D { ~D() {} };
struct CD { constexpr CD() {} ~CD() {} };
struct T { int a, b; };
struct R { int x; int &r; };
constexpr int ce() { return 4; }
const int k = 3;
int g;

// Constant initializers.
__thread C c1(1);
__thread C c2 = C(2);
__thread T t1;
__thread T t2 = T();
__thread T t3 = {};
__thread T t4 = {1, k};
__thread int i1 = ce();
__thread int i2 = k;
__thread int *p1 = &g;
__thread int &r1 = g;
__thread R r2{1, g};
__thread int a1[2] = {1, 2};
__thread long l1 = (long)3;
__thread int i3 = __extension__ 4;
__thread int *p2 = nullptr;
_Thread_local C c3(3);

// Dynamic initialization.
__thread int d1 = f();
__thread N d2(1);
__thread N d3;
_Thread_local N d4(1);
__thread int d5 = i1;
__thread int &d6 = i1;
__thread int *d7 = &i1;
__thread int d8{f()};
__thread T d9 = {1, f()};
__thread int d10[2] = {1, f()};
__thread R d11{1, i1};

// Non-trivial destruction.
__thread D x1;
__thread CD x2;
__thread D x3[2];

// `thread_local` and `extern` are not restricted.
thread_local N ok1(1);
thread_local D ok2;
extern __thread N ok3;
extern __thread D ok4;

void local() {
  static __thread N l1(1);
  static __thread int l2 = f();
  static __thread D l3;
  static __thread C l4(4);
}
