// [support.srclog.cons]: a library `source_location::current` that casts
// the `const void *` of `__builtin_source_location()` back to its
// implementation struct is a constant expression, also in a generic
// function and in a default member initializer (the line of its use). Found running GCC's C++
// testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line, _M_column; };
  const __impl *__ptr;
  constexpr source_location() : __ptr(nullptr) {}
  static consteval source_location current(const void *__p = __builtin_source_location()) {
    source_location __ret;
    __ret.__ptr = static_cast<const __impl *>(__p);
    return __ret;
  }
  constexpr unsigned line() const { return __ptr ? __ptr->_M_line : 0; }
  constexpr unsigned column() const { return __ptr ? __ptr->_M_column : 0; }
};
}
constexpr int g(auto...) { return std::source_location::current().column(); }
constexpr int f()        { return std::source_location::current().column(); }
static_assert(g() == f());
struct M { unsigned line = std::source_location::current().line(); };
static_assert(M{}.line == 26);
