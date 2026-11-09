// [class.copy.ctor]/11: copying a mutable member selects TemplateCtor(T&)
// with T = TemplateCtor, a non-trivial constructor, so the copy is not
// trivial. From clang's CXX/special/class.copy/p12-0x.cpp. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, triviality of the copy ignores the constructor actually selected for a mutable member
//@ expect: compile-ok
struct TemplateCtor {
  template <typename T> TemplateCtor(T &);
};
struct MutableTemplateCtorMember {
  mutable TemplateCtor mtc;
};
static_assert(!__is_trivially_constructible(MutableTemplateCtorMember, const MutableTemplateCtorMember &), "");
static_assert(__is_trivially_constructible(MutableTemplateCtorMember, MutableTemplateCtorMember &&), "");
