// [dcl.init.general]/6: zero-initializing a class, array or base subobject
// sets each pointer to data member in it to the null member pointer value
// (-1 under the Itanium ABI, not all-zero bytes).
//@ expect: run
//@ stdout: 1 1 1 1 1 1 1 1 1 1 1 1 1
//@ stdout: 0 0
extern "C" int printf(const char*, ...);

struct T { int x; int T::*p; };
struct B { int T::*p; int y; };
struct D : B { D() : B() {} int z; };
struct N { int a; T t[2]; };
struct W { W() = default; int T::*p; char c[40]; };
union U { int T::*p; int i; };
T g{};
T ga[3]{};
int T::*gp;
static int n(int T::*p) { return p == nullptr; }

int main() {
  T t{};
  T t2 = T();
  T* h = new T();
  T* arr = new T[4]();
  D d;
  N nn{};
  W w{};
  U u{};
  static T s{};
  int r[] = {n(t.p), n(t2.p), n(h->p), n(arr[3].p), n(d.p), n(nn.t[1].p),
             n(w.p), n(u.p), n(s.p), n(g.p), n(ga[2].p), n(gp), n(T().p)};
  for (int k = 0; k < 13; ++k) printf(k == 12 ? "%d\n" : "%d ", r[k]);
  T v{1, &T::x};
  printf("%d %d\n", n(v.p), v.*(v.p) - 1);
  delete h;
  delete[] arr;
}
