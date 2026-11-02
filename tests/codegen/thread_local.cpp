// [basic.stc.thread], [basic.start.dynamic]/7, [basic.start.term]/1,
// Itanium ABI §3.4: thread-local variables are `thread_local` globals
// read and written through @llvm.threadlocal.address. Dynamically
// initialized namespace-scope ones are initialized by __tls_init (guarded
// by __tls_guard) on a thread's first odr-use through their _ZTW wrapper,
// whose _ZTH alias names the initializer; a static data member, an inline
// variable and an extern declaration without a definition here get their
// own wrapper. Function-local ones are guarded without __cxa_guard (a
// thread cannot race itself), and destructors register with
// __cxa_thread_atexit. The address of a thread_local is not a constant
// ([expr.const]/14), so a reference bound to one is dynamically
// initialized; a temporary it extends is a thread_local _ZGR.
int f();
struct D { D(); ~D(); int v; };
struct S { static thread_local int m; static inline thread_local int n = f(); };
thread_local int c = 1;
thread_local int d = f();
static thread_local int s = f();
thread_local D dd;
extern thread_local int e;
thread_local int S::m = f();
inline thread_local int iv = f();
thread_local int &r = S::m;
thread_local const D &rd = D();
constinit thread_local int ci = 4;
extern thread_local constinit int eci;
thread_local int arr[2] = {f(), 2};
int use() { return c + d + s + dd.v + e + S::m + S::n + iv + r + rd.v + ci + eci + arr[1]; }
void set() { S::m = 3; ci = 5; arr[0] = 7; r = 2; d = 1; }
int *addr() { return &S::m; }
int loc() {
  static thread_local int l = f();
  thread_local D ld;
  thread_local int z = 5;
  return l + ld.v + z++;
}
