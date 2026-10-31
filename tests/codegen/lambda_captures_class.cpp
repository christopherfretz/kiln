// Class-type and array captures: capture copy construction, the closure
// destructor, init captures direct-initializing their member, and the
// EH cleanups of partially initialized closures and captured arrays.
struct N { int v; N(int x); N(const N&); ~N(); int get() const { return v; } };
int cls(N n) { auto l = [n] { return n.get(); }; return l(); }
int clsinit(int a) { auto l = [m = N(a)] { return m.get(); }; return l(); }
int nested_arr(int (&a)[2][2]) { return [a] { return a[1][1]; }(); }
int clsarr(N (&a)[2]) { return [a] { return a[1].get(); }(); }
struct T { int v; T(); T(const T&); ~T();
  int f() { return [*this] { return v; }(); }
  int g() { return [this] { return [this] { return v; }(); }(); } };
int useT() { T t; return t.f() + t.g(); }
int byref_cls(N& n) { return [&n] { return n.get(); }(); }
int ptrcap(int* p) { return [p] { return *p; }(); }
void store(int& out, int v) { [&out, v] { out = v; }(); }
int clsarr2(N (&a)[2][2]) { return [a] { return a[1][1].get(); }(); }
int twocls(N x, N y) { return [x, y] { return x.get() + y.get(); }(); }
