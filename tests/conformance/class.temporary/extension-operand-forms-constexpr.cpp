// The constant-evaluation twin of extension-operand-forms-run.cpp and
// extension-conditional-array-arms-run.cpp ([class.temporary]/6): the same
// extension, so a read through the reference inside the extended lifetime
// is constant and one after it is not. Only the chosen arm is destroyed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 75:15 error "constexpr variable 'x1' must be initialized by a constant expression"
//@ diag: 67:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 75:20 note "in call to 'member_ptr()'"
//@ diag: 76:15 error "constexpr variable 'x2' must be initialized by a constant expression"
//@ diag: 73:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 76:20 note "in call to 'chosen_arm()'"
struct Log {
  int v[40] = {};
  int n = 0;
  constexpr void add(int x) { v[n++] = x; }
  constexpr bool is(const int* e, int k) const {
    for (int i = 0; i < k; ++i)
      if (v[i] != e[i]) return false;
    return n == k;
  }
};
struct B { int v; int w; };
struct A {
  Log* l; B b; B arr[2]; int i;
  constexpr A(Log* l, int x) : l(l), b{x, x}, arr{{x, x}, {x, x}}, i(x) {}
  constexpr ~A() { l->add(-i); }
};
struct D : A { using A::A; };
struct R { const B& r; };
struct S { Log* l; int v; constexpr ~S() { l->add(-v); } };
struct X { S a[2]; };
constexpr bool c = true;

constexpr bool forms() {
  Log l;
  { const B& r = A(&l, 1).*(&A::b); l.add(r.v); }
  { const int& r = (A(&l, 2).*(&A::b)).v; l.add(r); }
  { int&& r = 1[A(&l, 3).arr].w; l.add(r); }
  { const int& r = ((void)0, ((void)0, A(&l, 4).b).v); l.add(r); }
  { const int& r = (c ? A(&l, 5).b : A(&l, 99).b).w; l.add(r); }
  { B&& r = !c ? static_cast<B&&>(A(&l, 98).b) : static_cast<B&&>(A(&l, 6).b); l.add(r.v); }
  { R r{A(&l, 7).*(&A::b)}; l.add(r.r.v); }
  { const A& r = static_cast<const A&>(D(&l, 8)); l.add(r.i); }
  { const D& r = static_cast<const D&>(static_cast<const A&>(D(&l, 9))); l.add(r.i); }
  { int&& r = const_cast<int&&>(A(&l, 10).*(&A::i)); l.add(r); }
  const int e[] = {1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6, -6, 7, -7, 8, -8, 9, -9, 10, -10};
  return l.is(e, 20);
}
static_assert(forms());

constexpr bool arms(bool k) {
  Log l;
  { const S (&r)[2] = k ? X{{{&l, 1}, {&l, 2}}}.a : X{{{&l, 3}, {&l, 4}}}.a; l.add(r[0].v); }
  { const S& r = (k ? X{{{&l, 5}, {&l, 6}}} : X{{{&l, 7}, {&l, 8}}}).a[1]; l.add(r.v); }
  const int t[] = {1, -2, -1, 6, -6, -5};
  const int f[] = {3, -4, -3, 8, -8, -7};
  return l.is(k ? t : f, 6);
}
static_assert(arms(true));
static_assert(arms(false));

constexpr int member_ptr() {
  Log l;
  const int* p;
  { const B& r = A(&l, 1).*(&A::b); p = &r.v; }
  return *p;
}
constexpr int chosen_arm() {
  Log l;
  const int* p;
  { const int& r = (c ? A(&l, 2).b : A(&l, 3).b).w; p = &r; }
  return *p;
}
constexpr int x1 = member_ptr();
constexpr int x2 = chosen_arm();
