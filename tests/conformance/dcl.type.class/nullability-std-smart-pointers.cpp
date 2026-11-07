// [dcl.type.class] (clang extension, nullability on classes): clang marks
// these standard library classes nullable without a written `_Nullable`
// (`inferNullableClassAttribute`): auto_ptr, coroutine_handle,
// exception_ptr, function, move_only_function, shared_ptr and unique_ptr,
// declared directly in `std` (inline namespaces skipped), templates or
// not. Another name, or one in a nested non-inline namespace, is not.
// abseil absl/base/nullability_test.cc (`absl_nonnull
// std::unique_ptr<int>`, WP-686). Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:11 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'std::weak_ptr<int>'"
//@ diag: 33:11 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'std::detail::unique_ptr'"
namespace std {
inline namespace v1 {
template <class T> struct unique_ptr {};
template <class T> struct shared_ptr {};
template <class T> struct weak_ptr {};
template <class F> struct function;
template <class R> struct function<R()> {};
template <class P = void> struct coroutine_handle {};
}
struct exception_ptr {};
template <class T> struct auto_ptr {};
template <class F> struct move_only_function {};
namespace detail { struct unique_ptr {}; }
}
using U = _Nonnull std::unique_ptr<int>;
using S = std::shared_ptr<int> _Nullable;
using F = _Nullable std::function<int()>;
using C = _Nonnull std::coroutine_handle<>;
using E = _Null_unspecified std::exception_ptr;
using W = _Nonnull std::weak_ptr<int>;
using D = _Nonnull std::detail::unique_ptr;
using A = _Nonnull std::auto_ptr<int>;
using M = _Nonnull std::move_only_function<int>;
template <class T> using UT = _Nonnull std::unique_ptr<T>;
UT<char> ut;
