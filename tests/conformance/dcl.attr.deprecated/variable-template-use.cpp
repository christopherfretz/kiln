// [dcl.attr.deprecated], [temp.expl.spec] (WP-432): a use of a
// specialization of a deprecated variable template warns with the
// template-id (libc++ `is_pod_v` / `is_trivial_v`, both attribute
// spellings, in an inline namespace), also from an instantiation; an
// explicit specialization is not deprecated by the primary; a use inside
// a deprecated function template does not warn.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 30:20 warning "'is_pod_v<int>' is deprecated"
//@ diag: 23:22 note "'is_pod_v<int>' has been explicitly marked deprecated here"
//@ diag: 31:20 warning "'is_trivial_v<int>' is deprecated: Consider using X instead."
//@ diag: 25:16 note "'is_trivial_v<int>' has been explicitly marked deprecated here"
//@ diag: 32:20 warning "'is_pod_d<int>' is deprecated: use is_trivially_copyable"
//@ diag: 22:29 note "'is_pod_d<int>' has been explicitly marked deprecated here"
//@ diag: 33:20 warning "'spec_v<int>' is deprecated"
//@ diag: 27:22 note "'spec_v<int>' has been explicitly marked deprecated here"
//@ diag: 35:53 warning "'is_pod_v<int>' is deprecated"
//@ diag: 36:15 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 23:22 note "'is_pod_v<int>' has been explicitly marked deprecated here"
namespace std { inline namespace __1 {
template <class T> struct is_pod { static constexpr bool value = __is_pod(T); };
template <class T> struct [[deprecated("use is_trivially_copyable")]] is_pod_d : is_pod<T> {};
template <class T> [[deprecated]] inline constexpr bool is_pod_v = __is_pod(T);
template <class T>
__attribute__((__deprecated__("Consider using X instead.")))
inline constexpr bool is_trivial_v = __is_trivially_copyable(T);
template <class T> [[deprecated]] constexpr bool spec_v = true;
template <> constexpr bool spec_v<long> = false;
} }
static_assert(std::is_pod_v<int>);
static_assert(std::is_trivial_v<int>);
static_assert(std::is_pod_d<int>::value);
static_assert(std::spec_v<int>);
static_assert(!std::spec_v<long>);
template <class T> constexpr bool g() { return std::is_pod_v<T>; }
static_assert(g<int>());
template <class T> [[deprecated]] constexpr bool g2() { return std::is_pod_v<T>; }
