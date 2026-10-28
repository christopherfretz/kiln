//@ mode: pp
//@ args: -stdlib=libc++
// The shape of a real translation unit's preprocessing: the whole libc++
// include graph (file lookup, include guards, large skipped regions), then a
// header-style configuration layer -- feature probing with `__has_include`,
// `__has_cpp_attribute` and feature-test macros, long `#if`/`#elif` chains,
// skipped blocks with nested directives, `#define`/`#undef` churn -- that the
// file re-includes 16 x 16 x 20 times so the total is well above timer noise.
#if !defined(HDR_L1)
#define HDR_L1
#include <algorithm>
#include <any>
#include <array>
#include <atomic>
#include <barrier>
#include <bit>
#include <bitset>
#include <cassert>
#include <cctype>
#include <cerrno>
#include <cfenv>
#include <cfloat>
#include <charconv>
#include <chrono>
#include <cinttypes>
#include <climits>
#include <clocale>
#include <cmath>
#include <compare>
#include <complex>
#include <concepts>
#include <condition_variable>
#include <coroutine>
#include <csetjmp>
#include <csignal>
#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <cuchar>
#include <cwchar>
#include <cwctype>
#include <deque>
#include <exception>
#include <execution>
#include <expected>
#include <filesystem>
#include <format>
#include <forward_list>
#include <fstream>
#include <functional>
#include <future>
#include <initializer_list>
#include <iomanip>
#include <ios>
#include <iosfwd>
#include <iostream>
#include <istream>
#include <iterator>
#include <latch>
#include <limits>
#include <list>
#include <locale>
#include <map>
#include <mdspan>
#include <memory>
#include <memory_resource>
#include <mutex>
#include <new>
#include <numbers>
#include <numeric>
#include <optional>
#include <ostream>
#include <print>
#include <queue>
#include <random>
#include <ranges>
#include <ratio>
#include <regex>
#include <scoped_allocator>
#include <semaphore>
#include <set>
#include <shared_mutex>
#include <source_location>
#include <span>
#include <sstream>
#include <stack>
#include <stdexcept>
#include <stop_token>
#include <streambuf>
#include <string>
#include <string_view>
#include <syncstream>
#include <system_error>
#include <thread>
#include <tuple>
#include <type_traits>
#include <typeindex>
#include <typeinfo>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <valarray>
#include <variant>
#include <vector>
#include <version>

#define HDR_CAT(a, b) HDR_CAT_I(a, b)
#define HDR_CAT_I(a, b) a##b
#define HDR_NS HDR_CAT(HDR_CAT(HDR_CAT(cfg, HDR_L2), HDR_L3), HDR_L4)

#define HDR_L2 0
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 1
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 2
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 3
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 4
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 5
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 6
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 7
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 8
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 9
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 a
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 b
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 c
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 d
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 e
#include "headers.cpp"
#undef HDR_L2
#define HDR_L2 f
#include "headers.cpp"
#undef HDR_L2

int main() { return 0; }

#elif !defined(HDR_IN_L2)
#define HDR_IN_L2
#define HDR_L3 0
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 1
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 2
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 3
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 4
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 5
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 6
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 7
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 8
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 9
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 a
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 b
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 c
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 d
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 e
#include "headers.cpp"
#undef HDR_L3
#define HDR_L3 f
#include "headers.cpp"
#undef HDR_L3
#undef HDR_IN_L2
#elif !defined(HDR_IN_L3)
#define HDR_IN_L3
#define HDR_L4 0
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 1
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 2
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 3
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 4
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 5
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 6
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 7
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 8
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 9
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 a
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 b
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 c
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 d
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 e
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 f
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 g
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 h
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 i
#include "headers.cpp"
#undef HDR_L4
#define HDR_L4 j
#include "headers.cpp"
#undef HDR_L4
#undef HDR_IN_L3
#else
// ---- one pass of the configuration layer ----
#if defined(__cplusplus) && __cplusplus >= 202002L
#  define HDR_CXX20 1
#  if __cplusplus > 202302L
#    define HDR_CXX26 1
#  elif __cplusplus == 202302L
#    define HDR_CXX23 1
#  else
#    define HDR_CXX20_ONLY 1
#  endif
#else
#  error "C++20 or later is required"
#endif

#if __has_include(<version>) && __has_include(<format>) && __has_include(<ranges>)
#  define HDR_HAS_CORE 1
#endif
#if __has_include(<stacktrace>)
#  define HDR_HAS_STACKTRACE 1
#elif __has_include(<experimental/stacktrace>)
#  define HDR_HAS_STACKTRACE 2
#else
#  define HDR_HAS_STACKTRACE 0
#endif
#if __has_include(<flat_map>) || __has_include(<generator>) || __has_include(<hazard_pointer>)
#  define HDR_HAS_NEW_CONTAINERS 1
#else
#  define HDR_HAS_NEW_CONTAINERS 0
#endif
#if __has_include("no/such/header.h") || __has_include(<no_such_header>)
#  error "phantom header"
#endif

#if __has_cpp_attribute(nodiscard) >= 201907L
#  define HDR_NODISCARD(msg) [[nodiscard(msg)]]
#elif __has_cpp_attribute(nodiscard)
#  define HDR_NODISCARD(msg) [[nodiscard]]
#else
#  define HDR_NODISCARD(msg)
#endif
#if __has_cpp_attribute(no_unique_address) && !defined(_MSC_VER)
#  define HDR_NO_UNIQUE [[no_unique_address]]
#else
#  define HDR_NO_UNIQUE
#endif
#if __has_cpp_attribute(likely) && __has_cpp_attribute(unlikely) && __has_cpp_attribute(assume)
#  define HDR_LIKELY [[likely]]
#else
#  define HDR_LIKELY
#endif

#if defined(__cpp_concepts) && __cpp_concepts >= 202002L && defined(__cpp_lib_concepts)
#  define HDR_CONCEPTS 1
#endif
#if defined(__cpp_lib_ranges) && __cpp_lib_ranges >= 201911L && (__cpp_lib_ranges_zip >= 202110L || !defined(__cpp_lib_ranges_zip))
#  define HDR_RANGES 1
#endif
#if defined(__cpp_lib_format) && (__cpp_lib_format >= 202106L) && defined(__cpp_lib_print)
#  define HDR_FORMAT 2
#elif defined(__cpp_lib_format)
#  define HDR_FORMAT 1
#else
#  define HDR_FORMAT 0
#endif
#if (defined(__cpp_lib_expected) && __cpp_lib_expected >= 202211L) + (defined(__cpp_lib_mdspan)) + (defined(__cpp_lib_optional) && __cpp_lib_optional >= 202110L) >= 2
#  define HDR_VOCAB 1
#endif
#if defined(__cpp_constexpr) && __cpp_constexpr >= 202306L
#  define HDR_CONSTEXPR_LEVEL 3
#elif defined(__cpp_constexpr) && __cpp_constexpr >= 202211L
#  define HDR_CONSTEXPR_LEVEL 2
#elif defined(__cpp_constexpr_dynamic_alloc)
#  define HDR_CONSTEXPR_LEVEL 1
#else
#  define HDR_CONSTEXPR_LEVEL 0
#endif

// Feature table, as generated into every configuration header.
#if defined(__cpp_lib_ranges) && __cpp_lib_ranges >= 201911L
#  define HDR_F_LIB_RANGES 1
#else
#  define HDR_F_LIB_RANGES 0
#endif
#if defined(__cpp_lib_format) && __cpp_lib_format >= 202106L
#  define HDR_F_LIB_FORMAT 1
#else
#  define HDR_F_LIB_FORMAT 0
#endif
#if defined(__cpp_lib_print) && __cpp_lib_print >= 202207L
#  define HDR_F_LIB_PRINT 1
#else
#  define HDR_F_LIB_PRINT 0
#endif
#if defined(__cpp_lib_expected) && __cpp_lib_expected >= 202211L
#  define HDR_F_LIB_EXPECTED 1
#else
#  define HDR_F_LIB_EXPECTED 0
#endif
#if defined(__cpp_lib_mdspan) && __cpp_lib_mdspan >= 202207L
#  define HDR_F_LIB_MDSPAN 1
#else
#  define HDR_F_LIB_MDSPAN 0
#endif
#if defined(__cpp_lib_optional) && __cpp_lib_optional >= 202110L
#  define HDR_F_LIB_OPTIONAL 1
#else
#  define HDR_F_LIB_OPTIONAL 0
#endif
#if defined(__cpp_lib_variant) && __cpp_lib_variant >= 202106L
#  define HDR_F_LIB_VARIANT 1
#else
#  define HDR_F_LIB_VARIANT 0
#endif
#if defined(__cpp_lib_span) && __cpp_lib_span >= 202002L
#  define HDR_F_LIB_SPAN 1
#else
#  define HDR_F_LIB_SPAN 0
#endif
#if defined(__cpp_lib_bit_cast) && __cpp_lib_bit_cast >= 201806L
#  define HDR_F_LIB_BIT_CAST 1
#else
#  define HDR_F_LIB_BIT_CAST 0
#endif
#if defined(__cpp_lib_bitops) && __cpp_lib_bitops >= 201907L
#  define HDR_F_LIB_BITOPS 1
#else
#  define HDR_F_LIB_BITOPS 0
#endif
#if defined(__cpp_lib_byteswap) && __cpp_lib_byteswap >= 202110L
#  define HDR_F_LIB_BYTESWAP 1
#else
#  define HDR_F_LIB_BYTESWAP 0
#endif
#if defined(__cpp_lib_chrono) && __cpp_lib_chrono >= 201907L
#  define HDR_F_LIB_CHRONO 1
#else
#  define HDR_F_LIB_CHRONO 0
#endif
#if defined(__cpp_lib_concepts) && __cpp_lib_concepts >= 202002L
#  define HDR_F_LIB_CONCEPTS 1
#else
#  define HDR_F_LIB_CONCEPTS 0
#endif
#if defined(__cpp_lib_constexpr_vector) && __cpp_lib_constexpr_vector >= 201907L
#  define HDR_F_LIB_CONSTEXPR_VECTOR 1
#else
#  define HDR_F_LIB_CONSTEXPR_VECTOR 0
#endif
#if defined(__cpp_lib_constexpr_string) && __cpp_lib_constexpr_string >= 201907L
#  define HDR_F_LIB_CONSTEXPR_STRING 1
#else
#  define HDR_F_LIB_CONSTEXPR_STRING 0
#endif
#if defined(__cpp_lib_constexpr_memory) && __cpp_lib_constexpr_memory >= 202202L
#  define HDR_F_LIB_CONSTEXPR_MEMORY 1
#else
#  define HDR_F_LIB_CONSTEXPR_MEMORY 0
#endif
#if defined(__cpp_lib_constexpr_algorithms) && __cpp_lib_constexpr_algorithms >= 201806L
#  define HDR_F_LIB_CONSTEXPR_ALGORITHMS 1
#else
#  define HDR_F_LIB_CONSTEXPR_ALGORITHMS 0
#endif
#if defined(__cpp_lib_int_pow2) && __cpp_lib_int_pow2 >= 202002L
#  define HDR_F_LIB_INT_POW2 1
#else
#  define HDR_F_LIB_INT_POW2 0
#endif
#if defined(__cpp_lib_jthread) && __cpp_lib_jthread >= 201911L
#  define HDR_F_LIB_JTHREAD 1
#else
#  define HDR_F_LIB_JTHREAD 0
#endif
#if defined(__cpp_lib_latch) && __cpp_lib_latch >= 201907L
#  define HDR_F_LIB_LATCH 1
#else
#  define HDR_F_LIB_LATCH 0
#endif
#if defined(__cpp_lib_semaphore) && __cpp_lib_semaphore >= 201907L
#  define HDR_F_LIB_SEMAPHORE 1
#else
#  define HDR_F_LIB_SEMAPHORE 0
#endif
#if defined(__cpp_lib_source_location) && __cpp_lib_source_location >= 201907L
#  define HDR_F_LIB_SOURCE_LOCATION 1
#else
#  define HDR_F_LIB_SOURCE_LOCATION 0
#endif
#if defined(__cpp_lib_span_initializer_list) && __cpp_lib_span_initializer_list >= 202311L
#  define HDR_F_LIB_SPAN_INITIALIZER_LIST 1
#else
#  define HDR_F_LIB_SPAN_INITIALIZER_LIST 0
#endif
#if defined(__cpp_lib_string_contains) && __cpp_lib_string_contains >= 202011L
#  define HDR_F_LIB_STRING_CONTAINS 1
#else
#  define HDR_F_LIB_STRING_CONTAINS 0
#endif
#if defined(__cpp_lib_to_underlying) && __cpp_lib_to_underlying >= 202102L
#  define HDR_F_LIB_TO_UNDERLYING 1
#else
#  define HDR_F_LIB_TO_UNDERLYING 0
#endif
#if defined(__cpp_lib_unreachable) && __cpp_lib_unreachable >= 202202L
#  define HDR_F_LIB_UNREACHABLE 1
#else
#  define HDR_F_LIB_UNREACHABLE 0
#endif
#if defined(__cpp_lib_move_only_function) && __cpp_lib_move_only_function >= 202110L
#  define HDR_F_LIB_MOVE_ONLY_FUNCTION 1
#else
#  define HDR_F_LIB_MOVE_ONLY_FUNCTION 0
#endif
#if defined(__cpp_lib_ranges_zip) && __cpp_lib_ranges_zip >= 202110L
#  define HDR_F_LIB_RANGES_ZIP 1
#else
#  define HDR_F_LIB_RANGES_ZIP 0
#endif
#if defined(__cpp_lib_ranges_chunk) && __cpp_lib_ranges_chunk >= 202202L
#  define HDR_F_LIB_RANGES_CHUNK 1
#else
#  define HDR_F_LIB_RANGES_CHUNK 0
#endif
#if defined(__cpp_lib_ranges_enumerate) && __cpp_lib_ranges_enumerate >= 202302L
#  define HDR_F_LIB_RANGES_ENUMERATE 1
#else
#  define HDR_F_LIB_RANGES_ENUMERATE 0
#endif
#if defined(__cpp_lib_flat_map) && __cpp_lib_flat_map >= 202207L
#  define HDR_F_LIB_FLAT_MAP 1
#else
#  define HDR_F_LIB_FLAT_MAP 0
#endif
#if defined(__cpp_lib_generator) && __cpp_lib_generator >= 202207L
#  define HDR_F_LIB_GENERATOR 1
#else
#  define HDR_F_LIB_GENERATOR 0
#endif
#if defined(__cpp_lib_stacktrace) && __cpp_lib_stacktrace >= 202011L
#  define HDR_F_LIB_STACKTRACE 1
#else
#  define HDR_F_LIB_STACKTRACE 0
#endif
#if defined(__cpp_lib_text_encoding) && __cpp_lib_text_encoding >= 202306L
#  define HDR_F_LIB_TEXT_ENCODING 1
#else
#  define HDR_F_LIB_TEXT_ENCODING 0
#endif
#if defined(__cpp_lib_saturation_arithmetic) && __cpp_lib_saturation_arithmetic >= 202311L
#  define HDR_F_LIB_SATURATION_ARITHMETIC 1
#else
#  define HDR_F_LIB_SATURATION_ARITHMETIC 0
#endif
#if defined(__cpp_if_consteval) && __cpp_if_consteval >= 202106L
#  define HDR_F_IF_CONSTEVAL 1
#else
#  define HDR_F_IF_CONSTEVAL 0
#endif
#if defined(__cpp_deducing_this) && __cpp_deducing_this >= 202110L
#  define HDR_F_DEDUCING_THIS 1
#else
#  define HDR_F_DEDUCING_THIS 0
#endif
#if defined(__cpp_pack_indexing) && __cpp_pack_indexing >= 202311L
#  define HDR_F_PACK_INDEXING 1
#else
#  define HDR_F_PACK_INDEXING 0
#endif
#if defined(__cpp_static_call_operator) && __cpp_static_call_operator >= 202207L
#  define HDR_F_STATIC_CALL_OPERATOR 1
#else
#  define HDR_F_STATIC_CALL_OPERATOR 0
#endif
#if defined(__cpp_multidimensional_subscript) && __cpp_multidimensional_subscript >= 202211L
#  define HDR_F_MULTIDIMENSIONAL_SUBSCRIPT 1
#else
#  define HDR_F_MULTIDIMENSIONAL_SUBSCRIPT 0
#endif
#if defined(__cpp_constexpr) && __cpp_constexpr >= 202306L
#  define HDR_F_CONSTEXPR 1
#else
#  define HDR_F_CONSTEXPR 0
#endif
#if defined(__cpp_consteval) && __cpp_consteval >= 202211L
#  define HDR_F_CONSTEVAL 1
#else
#  define HDR_F_CONSTEVAL 0
#endif
#if defined(__cpp_structured_bindings) && __cpp_structured_bindings >= 202403L
#  define HDR_F_STRUCTURED_BINDINGS 1
#else
#  define HDR_F_STRUCTURED_BINDINGS 0
#endif
#if defined(__cpp_variadic_friend) && __cpp_variadic_friend >= 202403L
#  define HDR_F_VARIADIC_FRIEND 1
#else
#  define HDR_F_VARIADIC_FRIEND 0
#endif
#if defined(__cpp_contracts) && __cpp_contracts >= 202502L
#  define HDR_F_CONTRACTS 1
#else
#  define HDR_F_CONTRACTS 0
#endif
#if defined(__cpp_reflection) && __cpp_reflection >= 202506L
#  define HDR_F_REFLECTION 1
#else
#  define HDR_F_REFLECTION 0
#endif
#if (HDR_F_LIB_RANGES + HDR_F_LIB_FORMAT + HDR_F_LIB_PRINT + HDR_F_LIB_EXPECTED + HDR_F_LIB_MDSPAN + HDR_F_LIB_OPTIONAL + HDR_F_LIB_VARIANT + HDR_F_LIB_SPAN + HDR_F_LIB_BIT_CAST + HDR_F_LIB_BITOPS + HDR_F_LIB_BYTESWAP + HDR_F_LIB_CHRONO + HDR_F_LIB_CONCEPTS + HDR_F_LIB_CONSTEXPR_VECTOR + HDR_F_LIB_CONSTEXPR_STRING + HDR_F_LIB_CONSTEXPR_MEMORY + HDR_F_LIB_CONSTEXPR_ALGORITHMS + HDR_F_LIB_INT_POW2 + HDR_F_LIB_JTHREAD + HDR_F_LIB_LATCH + HDR_F_LIB_SEMAPHORE + HDR_F_LIB_SOURCE_LOCATION + HDR_F_LIB_SPAN_INITIALIZER_LIST + HDR_F_LIB_STRING_CONTAINS) >= 20 && \
    (HDR_F_LIB_TO_UNDERLYING + HDR_F_LIB_UNREACHABLE + HDR_F_LIB_MOVE_ONLY_FUNCTION + HDR_F_LIB_RANGES_ZIP + HDR_F_LIB_RANGES_CHUNK + HDR_F_LIB_RANGES_ENUMERATE + HDR_F_LIB_FLAT_MAP + HDR_F_LIB_GENERATOR + HDR_F_LIB_STACKTRACE + HDR_F_LIB_TEXT_ENCODING + HDR_F_LIB_SATURATION_ARITHMETIC + HDR_F_IF_CONSTEVAL + HDR_F_DEDUCING_THIS + HDR_F_PACK_INDEXING + HDR_F_STATIC_CALL_OPERATOR + HDR_F_MULTIDIMENSIONAL_SUBSCRIPT + HDR_F_CONSTEXPR + HDR_F_CONSTEVAL + HDR_F_STRUCTURED_BINDINGS + HDR_F_VARIADIC_FRIEND + HDR_F_CONTRACTS + HDR_F_REFLECTION) >= 0
#  define HDR_MODERN 1
#else
#  define HDR_MODERN 0
#endif

#if defined(_WIN32) || defined(_WIN64)
#  define HDR_PLATFORM windows
#  if defined(_MSC_VER) && _MSC_VER < 1930
#    error "unsupported MSVC"
#  elif defined(__MINGW32__)
#    define HDR_MINGW 1
#    include <no_such_windows_header.h>
#  endif
#  pragma warning(push)
#  define HDR_EXPORT __declspec(dllexport)
#elif defined(__APPLE__) && defined(__MACH__)
#  define HDR_PLATFORM darwin
#  if !defined(__has_feature) || !__has_feature(cxx_rtti)
#    define HDR_NO_RTTI 1
#  endif
#  define HDR_EXPORT __attribute__((visibility("default")))
#elif defined(__linux__) || defined(__FreeBSD__) || defined(__NetBSD__) || defined(__OpenBSD__)
#  define HDR_PLATFORM unix
#  if defined(__GLIBC__) && (__GLIBC__ * 100 + __GLIBC_MINOR__) < 217
#    error "glibc too old"
#  endif
#  define HDR_EXPORT __attribute__((visibility("default")))
#else
#  define HDR_PLATFORM unknown
#  define HDR_EXPORT
#endif

#if 0
// A large disabled region, as in every real header: its directives must be
// tracked for nesting but never evaluated.
#  if HDR_UNDEFINED_A(1, 2) / 0
#    include <does/not/exist>
#    define HDR_NEVER(x) x ## ## x
#  elif garbage tokens ' " that never lex as a whole
#    error "never"
#  else
#    pragma never
#  endif
#  ifdef HDR_PLATFORM
#    ifndef HDR_EXPORT
#      if 1
#        line 1 "never"
#      endif
#    endif
#  endif
template <class T> struct never_instantiated { T t; static_assert(sizeof(T) == 0); };
#  if defined(A) && defined(B) && defined(C) && defined(D) && defined(E)
#    if (A + B) * (C - D) / E
#    endif
#  endif
#endif

namespace HDR_NS {
#if HDR_FORMAT > 1 && defined(HDR_RANGES) && defined(HDR_CONCEPTS)
inline constexpr int feature_level = 3 + HDR_CONSTEXPR_LEVEL;
#elif HDR_FORMAT > 0
inline constexpr int feature_level = 2;
#else
inline constexpr int feature_level = 1;
#endif
struct HDR_NODISCARD("pass result") Pass {
  HDR_NO_UNIQUE int level = feature_level + HDR_HAS_STACKTRACE + HDR_HAS_NEW_CONTAINERS + HDR_MODERN;
};
}  // namespace HDR_NS

#undef HDR_CXX20
#undef HDR_CXX26
#undef HDR_CXX23
#undef HDR_CXX20_ONLY
#undef HDR_HAS_CORE
#undef HDR_HAS_STACKTRACE
#undef HDR_HAS_NEW_CONTAINERS
#undef HDR_NODISCARD
#undef HDR_NO_UNIQUE
#undef HDR_LIKELY
#undef HDR_CONCEPTS
#undef HDR_RANGES
#undef HDR_FORMAT
#undef HDR_VOCAB
#undef HDR_CONSTEXPR_LEVEL
#undef HDR_PLATFORM
#undef HDR_EXPORT
#undef HDR_NO_RTTI
#undef HDR_F_LIB_RANGES
#undef HDR_F_LIB_FORMAT
#undef HDR_F_LIB_PRINT
#undef HDR_F_LIB_EXPECTED
#undef HDR_F_LIB_MDSPAN
#undef HDR_F_LIB_OPTIONAL
#undef HDR_F_LIB_VARIANT
#undef HDR_F_LIB_SPAN
#undef HDR_F_LIB_BIT_CAST
#undef HDR_F_LIB_BITOPS
#undef HDR_F_LIB_BYTESWAP
#undef HDR_F_LIB_CHRONO
#undef HDR_F_LIB_CONCEPTS
#undef HDR_F_LIB_CONSTEXPR_VECTOR
#undef HDR_F_LIB_CONSTEXPR_STRING
#undef HDR_F_LIB_CONSTEXPR_MEMORY
#undef HDR_F_LIB_CONSTEXPR_ALGORITHMS
#undef HDR_F_LIB_INT_POW2
#undef HDR_F_LIB_JTHREAD
#undef HDR_F_LIB_LATCH
#undef HDR_F_LIB_SEMAPHORE
#undef HDR_F_LIB_SOURCE_LOCATION
#undef HDR_F_LIB_SPAN_INITIALIZER_LIST
#undef HDR_F_LIB_STRING_CONTAINS
#undef HDR_F_LIB_TO_UNDERLYING
#undef HDR_F_LIB_UNREACHABLE
#undef HDR_F_LIB_MOVE_ONLY_FUNCTION
#undef HDR_F_LIB_RANGES_ZIP
#undef HDR_F_LIB_RANGES_CHUNK
#undef HDR_F_LIB_RANGES_ENUMERATE
#undef HDR_F_LIB_FLAT_MAP
#undef HDR_F_LIB_GENERATOR
#undef HDR_F_LIB_STACKTRACE
#undef HDR_F_LIB_TEXT_ENCODING
#undef HDR_F_LIB_SATURATION_ARITHMETIC
#undef HDR_F_IF_CONSTEVAL
#undef HDR_F_DEDUCING_THIS
#undef HDR_F_PACK_INDEXING
#undef HDR_F_STATIC_CALL_OPERATOR
#undef HDR_F_MULTIDIMENSIONAL_SUBSCRIPT
#undef HDR_F_CONSTEXPR
#undef HDR_F_CONSTEVAL
#undef HDR_F_STRUCTURED_BINDINGS
#undef HDR_F_VARIADIC_FRIEND
#undef HDR_F_CONTRACTS
#undef HDR_F_REFLECTION
#undef HDR_MODERN
#endif
