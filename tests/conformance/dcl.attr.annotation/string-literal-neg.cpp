// [dcl.attr.annotation]/2: an annotation's value is
// `std::meta::reflect_constant(V)`, so V must be a valid constant template
// argument; a pointer to (a subobject of) a string literal, directly or in
// a class member, is not ([temp.arg.nontype]/6.2), also after
// substitution. A pointer to a named array is fine. GCC 16.1 accepts these
// annotations (its `reflect_constant("abc")` throws); kiln follows the
// draft (WP-830), so this test has no GCC oracle.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 23:10 error "annotation value refers to a string literal, which is not a valid constant template argument"
//@ diag: 24:10 error "annotation value refers to a string literal, which is not a valid constant template argument"
//@ diag: 25:10 error "annotation value refers to a string literal, which is not a valid constant template argument"
//@ diag: 28:10 error "annotation value refers to a string literal, which is not a valid constant template argument"
//@ diag: 29:29 error "annotation value refers to a string literal, which is not a valid constant template argument"
//@ diag: 30:6 note "in instantiation of template class 'D<H>' requested here"
constexpr const char *p = "xyz";
struct H {
  const char *s;
};
constexpr char arr[] = "ok";
struct [[="abc"]] S {};
struct [[=p]] S2 {};
struct [[=H{"q"}]] S3 {};
struct [[=arr]] S4 {};
struct [[=H{arr}]] S5 {};
void f([[="x"]] int);
template <class T> struct [[=T{"d"}]] D {};
D<H> dh;
