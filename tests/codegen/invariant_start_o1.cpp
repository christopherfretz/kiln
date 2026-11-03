//@ args: -O1 -Xclang -disable-llvm-passes
// [dcl.type.cv]/4, [basic.start.dynamic]: at -O1, after the dynamic
// initializer of a const object whose type has no mutable member and
// needs no destruction, clang marks its storage invariant
// (`EmitDeclInvariant`: `llvm.invariant.start.p0(i64 <size>, ptr @g)`),
// for namespace-scope, inline, thread-local and static local variables
// and const arrays alike. A reference, a type with a mutable member or a
// non-trivial destructor, and a non-const object get no marker.
int f();
struct A { int x; A(int); };
struct D { int x; D(int); ~D(); };
struct M { mutable int x; M(int); };
const int ci = f();
const A ca(f());
const D cd(f());
const int arr[2] = {f(), 2};
const M cm(f());
int nc = f();
thread_local const int tl = f();
inline const int il = f();
const int& cr = f();
int sl() { static const int s = f(); static const A sa(f()); return s + sa.x; }
int use() { return ci + ca.x + cd.x + arr[0] + cm.x + tl + il + cr; }
