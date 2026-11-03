// [dcl.stc]/4 (clang `MergeVarDecl`): every declaration of a variable has
// the same thread storage kind. A thread-local redeclaration of a
// non-thread-local variable, the reverse, and a change between static TLS
// (`__thread`, `_Thread_local`) and dynamic TLS (`thread_local`) are
// errors noted at the previous declaration; the declarations are still
// merged (a second definition is also a redefinition). Same in clang 22
// and 19.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 40:18 error "thread-local declaration of 'a1' with dynamic initialization follows declaration with static initialization"
//@ diag: 39:21 note "previous declaration is here"
//@ diag: 42:14 error "thread-local declaration of 'a2' with static initialization follows declaration with dynamic initialization"
//@ diag: 41:25 note "previous declaration is here"
//@ diag: 46:14 error "thread-local declaration of 'a4' follows non-thread-local declaration"
//@ diag: 45:12 note "previous declaration is here"
//@ diag: 48:5 error "non-thread-local declaration of 'a5' follows thread-local declaration"
//@ diag: 47:21 note "previous declaration is here"
//@ diag: 50:18 error "thread-local declaration of 'a6' with dynamic initialization follows declaration with static initialization"
//@ diag: 49:14 note "previous definition is here"
//@ diag: 50:18 error "redefinition of 'a6'"
//@ diag: 49:14 note "previous definition is here"
//@ diag: 52:21 error "thread-local declaration of 'a7' follows non-thread-local declaration"
//@ diag: 51:5 note "previous definition is here"
//@ diag: 54:12 error "non-thread-local declaration of 'a8' follows thread-local declaration"
//@ diag: 53:14 note "previous definition is here"
//@ diag: 63:21 error "thread-local declaration of 's1' with dynamic initialization follows declaration with static initialization"
//@ diag: 57:23 note "previous declaration is here"
//@ diag: 64:17 error "thread-local declaration of 's2' with static initialization follows declaration with dynamic initialization"
//@ diag: 58:27 note "previous declaration is here"
//@ diag: 65:17 error "thread-local declaration of 's3' follows non-thread-local declaration"
//@ diag: 59:14 note "previous declaration is here"
//@ diag: 66:8 error "non-thread-local declaration of 's4' follows thread-local declaration"
//@ diag: 60:23 note "previous declaration is here"
//@ diag: 70:41 error "thread-local declaration of 'q' with dynamic initialization follows declaration with static initialization"
//@ diag: 69:49 note "previous declaration is here"
//@ diag: 73:38 error "thread-local declaration of 'g1' with dynamic initialization follows declaration with static initialization"
//@ diag: 72:14 note "previous definition is here"
extern __thread int a1;
thread_local int a1;
extern thread_local int a2;
__thread int a2;
extern _Thread_local int a3;
__thread int a3;
extern int a4;
__thread int a4;
extern __thread int a5;
int a5;
__thread int a6;
thread_local int a6;
int a7;
extern __thread int a7;
__thread int a8;
extern int a8;

struct M {
  static __thread int s1;
  static thread_local int s2;
  static int s3;
  static __thread int s4;
  static __thread int s5;
};
thread_local int M::s1 = 1;
__thread int M::s2 = 1;
__thread int M::s3 = 1;
int M::s4 = 1;
__thread int M::s5 = 1;

template <class T> struct Q { static __thread T q; };
template <class T> thread_local T Q<T>::q;

__thread int g1;
void use() { extern thread_local int g1; }
