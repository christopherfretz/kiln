// WP-1331: the C2y working draft (-std=c2y): C23's language with
// __STDC_VERSION__ 202400L (clang 19/22).
//@ std: c23
//@ args: -std=c2y
//@ expect: pp
//@ skip(gcc): clang's predefines and feature-test hooks
STDC __STDC__ HOSTED __STDC_HOSTED__ VERSION __STDC_VERSION__
UTF __STDC_UTF_16__ __STDC_UTF_32__ STRICT __STRICT_ANSI__
INLINE __GNUC_STDC_INLINE__ __GNUC_GNU_INLINE__
#ifdef __cplusplus
cplusplus __cplusplus
#endif
#ifdef __has_cpp_attribute
has_cpp_attribute
#endif
#ifdef __has_c_attribute
attrs __has_c_attribute(nodiscard) __has_c_attribute(deprecated) __has_c_attribute(__fallthrough__)
attrs __has_c_attribute(maybe_unused) __has_c_attribute(noreturn) __has_c_attribute(_Noreturn)
attrs __has_c_attribute(gnu::unused) __has_c_attribute(__gnu__::__packed__) __has_c_attribute(clang::overloadable)
attrs __has_c_attribute(likely) __has_c_attribute(no_unique_address)
#endif
feature __has_feature(c_static_assert) __has_feature(c_alignas) __has_feature(c_generic_selections)
feature __has_feature(c_thread_local) __has_feature(cxx_rtti) __has_feature(cxx_exceptions)
extension __has_extension(c_static_assert) __has_extension(c_generic_selections) __has_extension(cxx_rtti)
builtin __has_builtin(__builtin_expect) __has_builtin(__is_same) __has_builtin(__builtin_types_compatible_p)
keyword __is_identifier(class) __is_identifier(bool) __is_identifier(_Bool) __is_identifier(restrict)
keyword __is_identifier(typeof) __is_identifier(nullptr) __is_identifier(inline) __is_identifier(and)
