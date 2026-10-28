//@ expect: pp
// Feature-test operators in #if: __has_cpp_attribute, __has_attribute,
// __has_builtin, __has_feature, __has_extension and __is_identifier.
// Values are clang's; kiln must agree with the oracle's answers.
#if __has_cpp_attribute(nodiscard)
nodiscard __has_cpp_attribute(nodiscard);
#endif
#if __has_cpp_attribute(gnu::always_inline)
gnu_always_inline;
#endif
#if !__has_cpp_attribute(no_such_attribute)
no_such_attribute;
#endif
#if __has_attribute(always_inline) && __has_attribute(__always_inline__)
has_attribute;
#endif
#if __has_builtin(__builtin_expect) && !__has_builtin(__no_such_builtin)
has_builtin;
#endif
#if __has_feature(cxx_rvalue_references) && __has_extension(cxx_rvalue_references)
has_feature;
#endif
#if __is_identifier(foo) && !__is_identifier(int)
is_identifier;
#endif
__has_cpp_attribute(likely) __has_cpp_attribute(fallthrough)
