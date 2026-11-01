// [dcl.init.general]/16.6.2.2 (P0960): parenthesized aggregate
// initialization of locals, globals, temporaries, unions, arrays (a
// value-initialized tail; class elements by their default constructor,
// destroyed on unwind) and array new-expressions.
struct A { int x; int y; };
struct B { int x; long y; double z = 2.5; };
struct C : A { int z; };
struct D { A a; int b[2]; };
union U { int i; float f; };
struct S { S() : v(7) {} S(int x) : v(x) {} ~S(); int v; };
struct T { T() : v(3) {} int v; };
struct Z { int a; int b = 5; };
int g();
void f() {
  A a(3);
  A b(1, 2);
  B c(1, 2);
  C d(A{1, 2}, 3);
  int e[3](1, 2);
  int h[](4, 5, 6);
  D dd(A(1, 2));
  A t = A(g(), 4);
  const A& r = A(5);
}
constexpr int k() { A a(3, 4); int arr[3](1, 2); B b(1); return a.x + a.y + arr[1] + arr[2] + (int)b.z; }
int use_k() { return k(); }
A glob(7, 8);
int garr[4](1);
U g1(3);
U g2(2.5);
int use_u() { U u(4); return u.i; }
float use_uf(int x) { U u(x); return u.f; }
int use_arr() { S s[3](S(1)); return s[2].v; }
int use_z() { Z z[2](Z{1, 2}); return z[1].b; }
int* new_a() { return new int[3](1, 2); }
int* new_b(int n) { return new int[n](1, 2); }
int* new_c() { return new int[](1, 2, 3); }
T* new_d(int n) { return new T[n](T()); }
Z* new_e() { return new Z(9); }
