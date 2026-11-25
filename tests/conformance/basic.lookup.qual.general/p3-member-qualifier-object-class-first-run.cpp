// [basic.lookup.qual.general]/2-3, [temp.res.general]/1: in `t.N::m` the
// first component `N` is a member-qualified name whose lookup context is
// the object's type. On a type-dependent object that context is dependent
// at the definition, so `N` is found there by unqualified lookup only; in
// the specialization nothing is dependent, and `N` is looked up in the
// object's class first, the definition's `N` used only when the class has
// none. Reduced from real code (a tooling client was told an edit kept
// meaning while `f<W>` called the namespace's `B::f`); clang 19 and 22
// agree.
//@ expect: run
//@ strict-diags
//@ stdout: 2 2 2 2 30 2 2 4 2 2 2 2
extern "C" int printf(const char *, ...);

// A namespace and a member class of the same name.
namespace acme {
struct B { int f() { return 1; } };
template <class> struct X { int f() { return 3; } };
}
struct WB { int f() { return 2; } };
template <class> struct WX { int f() { return 4; } };
struct W : acme::B, WB, acme::X<int>, WX<int> {
  struct acme {
    using B = WB;
    using Only = WB;  // nothing named `Only` in namespace `acme`
    template <class U> using X = WX<U>;
  };
};
template <class T> int ns_vs_member(T t) { return t.acme::B::f(); }
template <class T> int ns_vs_member_ptr(T *p) { return p->acme::B::f(); }
// A later component found only in the class's `acme`.
template <class T> int only_in_class(T t) { return t.acme::Only::f(); }
// A later template-id, looked up in the class's `acme`.
template <class T> int later_template_id(T t) { return t.acme::template X<int>::f(); }

// A member typedef naming a base, against a namespace-scope typedef of
// the same name naming the other base.
struct B1 { int f() { return 1; } };
struct B2 { int f() { return 2; } };
using Base = B1;
struct V : B1, B2 { using Base = B2; };
template <class T> int typedef_base(T t) { return t.Base::f(); }
template <class T> int typedef_base_ptr(T *p) { return p->Base::f(); }

// The injected-class-name of a base found in the object's class, against
// an unrelated class of the same name at namespace scope.
namespace n { struct C { int f() { return 30; } }; }
struct C { int f() { return 31; } };
struct Y : n::C {};
template <class T> int injected(T t) { return t.C::f(); }

// A member of the object's class named like the template parameter.
struct A1 { int f() { return 1; } };
struct A2 { int f() { return 2; } };
struct Z : A1, A2 { using T = A2; };
template <class T> int param_name(T t) { return t.T::f(); }

// A specialization of a class template with the same parameter
// positions is not the current instantiation of another template, and a
// generic lambda's object is dependent too.
template <class T> struct S : T {};
template <class T> int other_template(S<T> s) { return s.acme::B::f(); }
template <class T> struct Q {
  template <class U> int m(S<U> &s) { return s.acme::B::f(); }
};

int main() {
  W w;
  V v;
  Y y;
  Z z;
  int r[12];
  r[0] = ns_vs_member(w);
  r[1] = ns_vs_member_ptr(&w);
  r[2] = typedef_base(v);
  r[3] = typedef_base_ptr(&v);
  r[4] = injected(y);
  r[5] = param_name(z);
  r[6] = only_in_class(w);
  r[7] = later_template_id(w);
  r[8] = w.acme::B::f();
  S<W> s;
  r[9] = other_template(s);
  auto l = [](auto t) { return t.acme::B::f(); };
  r[10] = l(w);
  Q<int> q;
  r[11] = q.m(s);
  for (int i = 0; i < 12; ++i)
    printf(i ? " %d" : "%d", r[i]);
  printf("\n");
}
