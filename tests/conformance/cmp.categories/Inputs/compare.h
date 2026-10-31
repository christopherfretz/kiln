// A libc++-shaped <compare> subset (std::__1, _CmpUnspecifiedParam,
// conversions between the categories) for the [class.compare] and
// [cmp.categories] tests: clang 19 accepts it as the comparison category
// types (CheckComparisonCategoryType).
#pragma once
namespace std {
inline namespace __1 {
using size_t = decltype(sizeof(0));
enum class _OrdResult : signed char { __less = -1, __equiv = 0, __greater = 1 };
enum class _NCmpResult : signed char { __unordered = -127 };
struct _CmpUnspecifiedParam {
  constexpr _CmpUnspecifiedParam(int _CmpUnspecifiedParam::*) noexcept {}
};
class partial_ordering {
  constexpr explicit partial_ordering(_OrdResult __v) noexcept : __value_(static_cast<signed char>(__v)) {}
  constexpr explicit partial_ordering(_NCmpResult __v) noexcept : __value_(static_cast<signed char>(__v)) {}
  constexpr bool __is_ordered() const noexcept { return __value_ != static_cast<signed char>(_NCmpResult::__unordered); }
public:
  static const partial_ordering less;
  static const partial_ordering equivalent;
  static const partial_ordering greater;
  static const partial_ordering unordered;
  friend constexpr bool operator==(partial_ordering, partial_ordering) noexcept = default;
  friend constexpr bool operator==(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__is_ordered() && __v.__value_ == 0; }
  friend constexpr bool operator<(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__is_ordered() && __v.__value_ < 0; }
  friend constexpr bool operator<=(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__is_ordered() && __v.__value_ <= 0; }
  friend constexpr bool operator>(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__is_ordered() && __v.__value_ > 0; }
  friend constexpr bool operator>=(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__is_ordered() && __v.__value_ >= 0; }
  friend constexpr partial_ordering operator<=>(partial_ordering __v, _CmpUnspecifiedParam) noexcept { return __v; }
private:
  signed char __value_;
};
inline constexpr partial_ordering partial_ordering::less(_OrdResult::__less);
inline constexpr partial_ordering partial_ordering::equivalent(_OrdResult::__equiv);
inline constexpr partial_ordering partial_ordering::greater(_OrdResult::__greater);
inline constexpr partial_ordering partial_ordering::unordered(_NCmpResult::__unordered);
class weak_ordering {
  constexpr explicit weak_ordering(_OrdResult __v) noexcept : __value_(static_cast<signed char>(__v)) {}
public:
  static const weak_ordering less;
  static const weak_ordering equivalent;
  static const weak_ordering greater;
  constexpr operator partial_ordering() const noexcept {
    return __value_ == 0 ? partial_ordering::equivalent : (__value_ < 0 ? partial_ordering::less : partial_ordering::greater);
  }
  friend constexpr bool operator==(weak_ordering, weak_ordering) noexcept = default;
  friend constexpr bool operator==(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ == 0; }
  friend constexpr bool operator<(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ < 0; }
  friend constexpr bool operator<=(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ <= 0; }
  friend constexpr bool operator>(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ > 0; }
  friend constexpr bool operator>=(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ >= 0; }
  friend constexpr weak_ordering operator<=>(weak_ordering __v, _CmpUnspecifiedParam) noexcept { return __v; }
private:
  signed char __value_;
};
inline constexpr weak_ordering weak_ordering::less(_OrdResult::__less);
inline constexpr weak_ordering weak_ordering::equivalent(_OrdResult::__equiv);
inline constexpr weak_ordering weak_ordering::greater(_OrdResult::__greater);
class strong_ordering {
  constexpr explicit strong_ordering(_OrdResult __v) noexcept : __value_(static_cast<signed char>(__v)) {}
public:
  static const strong_ordering less;
  static const strong_ordering equal;
  static const strong_ordering equivalent;
  static const strong_ordering greater;
  constexpr operator partial_ordering() const noexcept {
    return __value_ == 0 ? partial_ordering::equivalent : (__value_ < 0 ? partial_ordering::less : partial_ordering::greater);
  }
  constexpr operator weak_ordering() const noexcept {
    return __value_ == 0 ? weak_ordering::equivalent : (__value_ < 0 ? weak_ordering::less : weak_ordering::greater);
  }
  friend constexpr bool operator==(strong_ordering, strong_ordering) noexcept = default;
  friend constexpr bool operator==(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ == 0; }
  friend constexpr bool operator<(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ < 0; }
  friend constexpr bool operator<=(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ <= 0; }
  friend constexpr bool operator>(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ > 0; }
  friend constexpr bool operator>=(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v.__value_ >= 0; }
  friend constexpr strong_ordering operator<=>(strong_ordering __v, _CmpUnspecifiedParam) noexcept { return __v; }
private:
  signed char __value_;
};
inline constexpr strong_ordering strong_ordering::less(_OrdResult::__less);
inline constexpr strong_ordering strong_ordering::equal(_OrdResult::__equiv);
inline constexpr strong_ordering strong_ordering::equivalent(_OrdResult::__equiv);
inline constexpr strong_ordering strong_ordering::greater(_OrdResult::__greater);
}
}
