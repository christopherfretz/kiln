// [class.copy.assign]/9: the implicit copy assignment is trivial only if
// the operator selected for each subobject is; a mutable member is
// assigned from a non-const lvalue, which selects a template operator=
// taking T& here. A mutable member of a trivially copyable class, and a
// class containing a non-trivial one, follow the selected member too.
// WP-958 sibling of class.copy.ctor/mutable-member-template-ctor-not-trivial.cpp.
//@ std: c++20
//@ expect: compile-ok
struct TemplateAssign {
  template <typename T> TemplateAssign &operator=(T &);
};
struct MutableAssign {
  mutable TemplateAssign m;
};
static_assert(!__is_trivially_assignable(MutableAssign &, const MutableAssign &), "");
static_assert(__is_trivially_assignable(MutableAssign &, MutableAssign &&), "");
struct Plain {
  int i;
};
struct MutablePlain {
  mutable Plain p;
};
static_assert(__is_trivially_assignable(MutablePlain &, const MutablePlain &), "");
static_assert(__is_trivially_constructible(MutablePlain, const MutablePlain &), "");
struct Outer {
  MutableAssign a;
};
static_assert(!__is_trivially_assignable(Outer &, const Outer &), "");
static_assert(!__is_trivially_copyable(Outer), "");
