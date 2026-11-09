// C++03 [class.union]/1: members of trivial class types (and arrays of
// them), pointers to non-trivial classes, a non-trivial member of a
// struct (not an anonymous one), and a template whose member type is
// dependent and never instantiated with a non-trivial class (WP-1042).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++98
struct Trivial { int i; void f(); static int s; };
struct Nested { Trivial t[2]; };
struct Ctor { Ctor(); };
union U {
  Trivial t;
  Nested n[3];
  Ctor *p;
  int i;
};
struct S {
  struct { Trivial a; int b; };
  union { Nested c; int d; };
  Ctor member;
};
template <class B> union T { int a; B b; };
T<Trivial> tt;
template <class B> struct Unused { union { B b; }; };
