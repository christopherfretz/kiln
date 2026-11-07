// [dcl.fct.def.general]/8 (implementation-defined `__PRETTY_FUNCTION__`,
// clang's TemplateArgument::print): a template argument of an enumeration
// type whose underlying type is `bool` prints as its enumerator (`B::Nay`)
// as any other enumeration's does, never as the integer `1`. Enumeration
// reflection libraries read enumerator names from these strings.
// Verified against clang 19.
//@ expect: compile-ok
constexpr bool eq(const char* a, const char* b) {
  while (*a && *a == *b) {
    ++a;
    ++b;
  }
  return *a == *b;
}
enum class B : bool { Yay, Nay };
enum U : bool { Q };
template <auto V> constexpr const char* f() { return __PRETTY_FUNCTION__; }
static_assert(eq(f<B::Nay>(), "const char *f() [V = B::Nay]"));
static_assert(eq(f<static_cast<B>(false)>(), "const char *f() [V = B::Yay]"));
static_assert(eq(f<Q>(), "const char *f() [V = Q]"));
static_assert(eq(f<true>(), "const char *f() [V = true]"));
