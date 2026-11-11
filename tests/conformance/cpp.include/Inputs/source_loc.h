// Source-location builtins in an included header's bodies and default
// arguments (the shape of <source_location>).
#pragma once
#include <source_location>

namespace sl {
inline unsigned line_here() { return __builtin_LINE(); }
inline const char *func_here() { return __builtin_FUNCTION(); }
constexpr unsigned column_here() { return __builtin_COLUMN(); }
template <class T> const char *tfunc() { return __builtin_FUNCTION(); }
inline unsigned caller_line(unsigned l = __builtin_LINE()) { return l; }
template <class T = int> unsigned tline(unsigned l = __builtin_LINE()) { return l; }
inline std::source_location at(std::source_location s) { return s; }
inline unsigned use_line() { return caller_line(); }
struct Member {
  unsigned l = __builtin_LINE();
};
inline unsigned member_line() { return Member{}.l; }
} // namespace sl
