// [basic.lookup.qual.general]/2-3: the forms of a member qualifier whose
// first component keeps what the definition found, next to those the
// object's class decides. clang 19 and 22 agree on every value.
// - `t.TB<int>::f()` on a type-dependent object: the template name is
//   found where the template is defined (clang's `isTemplateName` with a
//   dependent object type) and kept; on an object of class type the
//   class's member template `TB` comes first.
// - `this->acme::B::f()` and `p->acme::B::f()` with `p` the current
//   instantiation: the class is known at the definition (it has nothing
//   named `acme` until instantiated), so the namespace stays.
// - A name the object's class does not have: the definition's binding.
// - `t.T::~T()`: the destructor of the object's class, whatever `T` names
//   in that class.
//@ expect: run
//@ strict-diags
//@ stdout: 10 20 1 1 1
//@ stdout: ~W ~A2 ~A1 ~V
extern "C" int printf(const char *, ...);
void *operator new(decltype(sizeof 0), void *p) noexcept;
void *operator new(decltype(sizeof 0), void *p) noexcept { return p; }

template <class> struct TB { int f() { return 10; } };
template <class> struct TB2 { int f() { return 20; } };
struct X : TB<int>, TB2<int> { template <class U> using TB = TB2<U>; };
template <class T> int template_id(T t) { return t.TB<int>::f(); }

namespace acme { struct B { int f() { return 1; } }; }
struct WB { int f() { return 2; } };
struct W : acme::B, WB { struct acme { using B = WB; }; };
template <class T> struct S : T {
  int g() { return this->acme::B::f(); }
  int h(S *p) { return p->acme::B::f(); }
};

struct U : acme::B {};
template <class T> int not_in_class(T t) { return t.acme::B::f(); }

struct A1 { ~A1() { printf(" ~A1"); } };
struct A2 { ~A2() { printf(" ~A2"); } };
struct D : A1, A2 { using T = A2; ~D() { printf("~W"); } };
struct V { ~V() { printf(" ~V"); } };
template <class T> void destroy(T &t) { t.T::~T(); }

int main() {
  X x;
  S<W> s;
  U u;
  int r[5];
  r[0] = template_id(x);
  r[1] = x.TB<int>::f();
  r[2] = s.g();
  r[3] = s.h(&s);
  r[4] = not_in_class(u);
  for (int i = 0; i < 5; ++i)
    printf(i ? " %d" : "%d", r[i]);
  printf("\n");
  alignas(D) alignas(V) unsigned char buf[sizeof(D) + sizeof(V)];
  D *d = new (buf) D;
  destroy(*d);
  V *v = new (buf) V;
  destroy(*v);
  int i = 0;
  destroy(i);
  printf("\n");
}
