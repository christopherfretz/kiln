// [basic.link]/15, [basic.link]/8, [basic.def.odr]/11: a class template
// specialization has linkage whatever its template arguments, so a
// variable template specialization `extern const wrapper<T> v<T>` taken
// with a local class `T` is not the error for an entity whose type has no
// linkage (clang `err_undefined_internal_type`); it is merely not
// externally visible (`-Wundefined-internal`, silenced here by the pragma
// as the library this is reduced from does). The variable is named only to
// form a constant pointer to a member in a template argument, so the
// program links without a definition. Reduced from a field-name reflection
// library (WP-1224). Matches clang 19 and 22.
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wundefined-internal"
#pragma clang diagnostic ignored "-Wundefined-var-template"
template <class T> struct wrapper { const T value; };
template <class T> extern const wrapper<T> fake_var;
template <class T> constexpr const T& fake_object() noexcept { return fake_var<T>.value; }
#pragma clang diagnostic pop

extern "C" int printf(const char*, ...);
template <auto* P> struct member_tag { static constexpr bool ok = P != nullptr; };
template <class T> constexpr bool has_y() { return member_tag<&fake_object<T>().y>::ok; }

namespace { struct A { int x, y; }; }
inline bool in_inline() { struct B { int x, y; }; return has_y<B>(); }

int main() {
  struct L { int x; long y; };
  printf("%d %d %d\n", has_y<L>(), has_y<A>(), in_inline());
}
