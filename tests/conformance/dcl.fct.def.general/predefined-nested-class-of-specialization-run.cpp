// [dcl.fct.def.general]/8, clang `TypePrinter::AppendScope`: a member class
// of a class template specialization prints its scope with the
// specialization's converted arguments, which are canonical types and so
// fully scoped (`a::S<b::P>::N`, not `a::S<P>::N`). This holds for an
// implicit instantiation of the primary template, an explicit
// specialization and an instantiation of a partial specialization.
// reflect-cpp derives JSON schema names from these strings (its
// `json.test_reflector_schema` test).
//@ expect: run
//@ strict-diags
//@ stdout: f() [T = a::S<b::P>::N]
//@ stdout: f() [T = a::R<b::P>::N]
//@ stdout: f() [T = a::R<b::Q<int>>::N]
//@ stdout: f() [T = a::R<b::Q<b::P>>::N]
//@ stdout: f() [T = a::S<a::S<b::P>::N>::N]
extern "C" int printf(const char*, ...);
namespace a {
template <class T> struct R {};
template <class T> struct S { struct N {}; };
}  // namespace a
namespace b {
struct P {};
template <class T> struct Q {};
}  // namespace b
namespace a {
template <> struct R<b::P> { struct N {}; };
template <class T> struct R<b::Q<T>> { struct N {}; };
}  // namespace a
template <class T> const char* f() { return __PRETTY_FUNCTION__ + 12; }
int main() {
  const char* r[] = {f<a::S<b::P>::N>(), f<a::R<b::P>::N>(), f<a::R<b::Q<int>>::N>(),
                     f<a::R<b::Q<b::P>>::N>(), f<a::S<a::S<b::P>::N>::N>()};
  for (const char* s : r) printf("%s\n", s);
}
