// kiln's <meta>-shaped header for the reflection tests (-freflection;
// spec/reflection.md §1.3). Not the real <meta> (a later WP): the names
// the implemented work packages answer, declared where P2996R13 and
// P3560R2 declare them. The functions with no body are consteval
// intrinsics the Oven dispatches by qualified name and signature
// (spec/reflection.md §3).
//
// The class types the intrinsics return (vector, string_view,
// u8string_view, source_location) come from the real standard library
// when one of its headers was included before this one (`_LIBCPP_VERSION`
// or `__GLIBCXX__` defined): include standard headers first. Otherwise
// this header declares minimal stand-ins of the same layout (the Oven
// matches results by layout, §3.5). The stand-in vector is move-only.
#pragma once

#if defined(_LIBCPP_VERSION) || defined(__GLIBCXX__)
#include <exception>
#include <initializer_list>
#include <source_location>
#include <string_view>
#include <vector>
#else
namespace std {
using __kiln_size_t = decltype(sizeof 0);

// A pointer and a size (libc++'s layout); the compiler fills them from
// the backing array ([dcl.init.list]/5).
template <class E>
class initializer_list {
  const E* __begin_;
  __kiln_size_t __size_;

 public:
  using value_type = E;
  using size_type = __kiln_size_t;
  constexpr initializer_list() noexcept : __begin_(nullptr), __size_(0) {}
  constexpr __kiln_size_t size() const noexcept { return __size_; }
  constexpr const E* begin() const noexcept { return __begin_; }
  constexpr const E* end() const noexcept { return __begin_ + __size_; }
};

class exception {
 public:
  constexpr exception() noexcept = default;
  constexpr exception(const exception&) noexcept = default;
  constexpr exception& operator=(const exception&) noexcept = default;
  constexpr virtual ~exception() = default;
  constexpr virtual const char* what() const noexcept { return "std::exception"; }
};

template <class T>
struct allocator {
  using value_type = T;
  constexpr allocator() noexcept = default;
  template <class U>
  constexpr allocator(const allocator<U>&) noexcept {}
  constexpr T* allocate(__kiln_size_t n) { return static_cast<T*>(::operator new(n * sizeof(T))); }
  constexpr void deallocate(T* p, __kiln_size_t) noexcept { ::operator delete(p); }
};

// Begin, end and end-of-capacity pointers (libc++'s and libstdc++'s
// layout). Read-only: elements are only made by the intrinsics.
template <class T, class A = allocator<T>>
class vector {
  T* __begin_ = nullptr;
  T* __end_ = nullptr;
  T* __cap_ = nullptr;

 public:
  using value_type = T;
  using size_type = __kiln_size_t;
  using iterator = const T*;
  using const_iterator = const T*;
  constexpr vector() noexcept = default;
  vector(const vector&) = delete;
  constexpr vector(vector&& o) noexcept : __begin_(o.__begin_), __end_(o.__end_), __cap_(o.__cap_) {
    o.__begin_ = o.__end_ = o.__cap_ = nullptr;
  }
  vector& operator=(const vector&) = delete;
  constexpr ~vector() {
    if (__begin_) A().deallocate(__begin_, __kiln_size_t(__cap_ - __begin_));
  }
  constexpr size_type size() const noexcept { return __kiln_size_t(__end_ - __begin_); }
  constexpr size_type capacity() const noexcept { return __kiln_size_t(__cap_ - __begin_); }
  constexpr bool empty() const noexcept { return __begin_ == __end_; }
  constexpr const T* data() const noexcept { return __begin_; }
  constexpr const T* begin() const noexcept { return __begin_; }
  constexpr const T* end() const noexcept { return __end_; }
  constexpr const T& operator[](size_type i) const noexcept { return __begin_[i]; }
  constexpr const T& front() const noexcept { return *__begin_; }
  constexpr const T& back() const noexcept { return __end_[-1]; }
};

template <class C>
class basic_string_view {
  const C* __data_ = nullptr;
  __kiln_size_t __size_ = 0;

 public:
  using value_type = C;
  using size_type = __kiln_size_t;
  constexpr basic_string_view() noexcept = default;
  constexpr basic_string_view(const C* s) noexcept : __data_(s) {
    while (s[__size_]) ++__size_;
  }
  constexpr basic_string_view(const C* s, size_type n) noexcept : __data_(s), __size_(n) {}
  constexpr const C* data() const noexcept { return __data_; }
  constexpr size_type size() const noexcept { return __size_; }
  constexpr size_type length() const noexcept { return __size_; }
  constexpr bool empty() const noexcept { return __size_ == 0; }
  constexpr const C* begin() const noexcept { return __data_; }
  constexpr const C* end() const noexcept { return __data_ + __size_; }
  constexpr const C& operator[](size_type i) const noexcept { return __data_[i]; }
  friend constexpr bool operator==(basic_string_view a, basic_string_view b) noexcept {
    if (a.__size_ != b.__size_) return false;
    for (size_type i = 0; i < a.__size_; ++i)
      if (a.__data_[i] != b.__data_[i]) return false;
    return true;
  }
};
using string_view = basic_string_view<char>;
using u8string_view = basic_string_view<char8_t>;

// One pointer to a static `__impl` (libc++'s and libstdc++'s layout;
// the shape __builtin_source_location requires).
class source_location {
  struct __impl {
    const char* _M_file_name;
    const char* _M_function_name;
    unsigned _M_line;
    unsigned _M_column;
  };
  using __bsl_ty = decltype(__builtin_source_location());
  const __impl* __ptr_ = nullptr;

 public:
  static consteval source_location current(__bsl_ty p = __builtin_source_location()) noexcept {
    source_location s;
    s.__ptr_ = static_cast<const __impl*>(p);
    return s;
  }
  constexpr source_location() noexcept = default;
  constexpr unsigned line() const noexcept { return __ptr_ ? __ptr_->_M_line : 0; }
  constexpr unsigned column() const noexcept { return __ptr_ ? __ptr_->_M_column : 0; }
  constexpr const char* file_name() const noexcept { return __ptr_ ? __ptr_->_M_file_name : ""; }
  constexpr const char* function_name() const noexcept {
    return __ptr_ ? __ptr_->_M_function_name : "";
  }
};
}  // namespace std
#endif

namespace std::meta {
using info = decltype(^^int);

// [meta.reflection.queries]
consteval info type_of(info r);

// [meta.reflection.names]
consteval bool has_identifier(info r);
consteval std::string_view identifier_of(info r);
consteval std::u8string_view u8identifier_of(info r);
consteval std::string_view display_string_of(info r);
consteval std::u8string_view u8display_string_of(info r);
consteval std::source_location source_location_of(info r);

// [meta.reflection.substitute] (WP-482). The arguments are any
// contiguous range of info (a braced list, an array, a vector).
consteval info template_of(info r);
consteval std::vector<info> template_arguments_of(info r);
template <class R = std::initializer_list<info>>
consteval bool can_substitute(info templ, R&& arguments);
template <class R = std::initializer_list<info>>
consteval info substitute(info templ, R&& arguments);

// The spike's name query (identifier_of as a NTBS; retired by WP-484).
consteval const char* name_of(info r);

// [meta.reflection.exception] (P3560R2): what a failing intrinsic throws.
// The message is copied (constexpr new), so a copy outlives its source.
// The base is std::exception where that is usable in a constant
// expression (P3378 constexpr exceptions, or the stand-in above); a
// standard library without it (libc++ 19, libstdc++ 14) gets a local
// polymorphic base with the same what().
#if (defined(_LIBCPP_VERSION) || defined(__GLIBCXX__)) && \
    !(defined(__cpp_lib_constexpr_exceptions) && __cpp_lib_constexpr_exceptions >= 202411L)
class __kiln_exception_base {
 public:
  constexpr __kiln_exception_base() noexcept = default;
  constexpr __kiln_exception_base(const __kiln_exception_base&) noexcept = default;
  constexpr __kiln_exception_base& operator=(const __kiln_exception_base&) noexcept = default;
  constexpr virtual ~__kiln_exception_base() = default;
  constexpr virtual const char* what() const noexcept { return "std::meta::exception"; }
};
#else
using __kiln_exception_base = std::exception;
#endif

class exception : public __kiln_exception_base {
  char* __what_ = nullptr;
  char8_t* __u8what_ = nullptr;
  decltype(sizeof 0) __size_ = 0;
  info __from_;
  std::source_location __where_;

  template <class C>
  constexpr void __copy(const C* s, decltype(sizeof 0) n) {
    __size_ = n;
    __what_ = new char[n + 1];
    __u8what_ = new char8_t[n + 1];
    for (decltype(sizeof 0) i = 0; i < n; ++i) {
      __what_[i] = char(s[i]);
      __u8what_[i] = char8_t(s[i]);
    }
    __what_[n] = 0;
    __u8what_[n] = 0;
  }

 public:
  consteval exception(std::u8string_view what, info from,
                      std::source_location where = std::source_location::current()) noexcept
      : __from_(from), __where_(where) {
    __copy(what.data(), what.size());
  }
  consteval exception(std::string_view what, info from,
                      std::source_location where = std::source_location::current()) noexcept
      : __from_(from), __where_(where) {
    __copy(what.data(), what.size());
  }
  consteval exception(const exception& o) noexcept
      : __kiln_exception_base(o), __from_(o.__from_), __where_(o.__where_) {
    __copy(o.__u8what_, o.__size_);
  }
  consteval exception& operator=(const exception& o) noexcept {
    if (this != &o) {
      delete[] __what_;
      delete[] __u8what_;
      __copy(o.__u8what_, o.__size_);
      __from_ = o.__from_;
      __where_ = o.__where_;
    }
    return *this;
  }
  constexpr ~exception() override {
    delete[] __what_;
    delete[] __u8what_;
  }
  constexpr const char* what() const noexcept override { return __what_; }
  consteval std::u8string_view u8what() const noexcept { return {__u8what_, __size_}; }
  consteval info from() const noexcept { return __from_; }
  consteval std::source_location where() const noexcept { return __where_; }
};

// [meta.reflection.layout]: offset_of's result.
struct member_offset {
  decltype((char*)0 - (char*)0) bytes;
  decltype((char*)0 - (char*)0) bits;
  constexpr decltype((char*)0 - (char*)0) total_bits() const { return 8 * bytes + bits; }
};

// The designating class of `via(cls)`: `cls` if it is null or a complete
// class type, else a std::meta::exception (WP-478).
consteval info __kiln_access_via(info cls);

// [meta.reflection.access.context]: a scope and a designating class
// (null: none). unchecked() is the (null, null) context, which checks
// nothing (spec/reflection-members.md §3).
class access_context {
  info __scope_;
  info __designating_;
  consteval access_context(info s, info d) noexcept : __scope_(s), __designating_(d) {}

 public:
  access_context() = delete;
  static consteval access_context current() noexcept;
  static consteval access_context unprivileged() noexcept { return {^^::, info()}; }
  static consteval access_context unchecked() noexcept { return {info(), info()}; }
  consteval access_context via(info cls) const { return {__scope_, __kiln_access_via(cls)}; }
  consteval info scope() const noexcept { return __scope_; }
  consteval info designating_class() const noexcept { return __designating_; }
};

// The materialization recipes under test (spec/reflection.md §3.5):
// a vector<info> of any contiguous range of info, a member_offset and an
// access_context made by the Oven from their members.
template <class R>
consteval std::vector<info> __kiln_reflection_range(R&& r);
consteval member_offset __kiln_member_offset(decltype((char*)0 - (char*)0) bytes,
                                             decltype((char*)0 - (char*)0) bits);
consteval access_context __kiln_access_context(info scope, info designating);

// [meta.reflection.access.context]: the scope queries (P2996R13).
consteval info current_function();
consteval info current_class();
consteval info current_namespace();

// [meta.reflection.access.queries]
consteval bool is_accessible(info r, access_context ctx);
consteval bool has_inaccessible_nonstatic_data_members(info r, access_context ctx);
consteval bool has_inaccessible_bases(info r, access_context ctx);
consteval bool has_inaccessible_subobjects(info r, access_context ctx);

// [meta.reflection.member.queries]
consteval std::vector<info> members_of(info r, access_context ctx);
consteval std::vector<info> bases_of(info r, access_context ctx);
consteval std::vector<info> static_data_members_of(info r, access_context ctx);
consteval std::vector<info> nonstatic_data_members_of(info r, access_context ctx);
consteval std::vector<info> subobjects_of(info r, access_context ctx);
consteval std::vector<info> enumerators_of(info type_enum);
consteval std::vector<info> parameters_of(info r);

// A test-header convenience, not IS: nonstatic_data_members_of(R) copied
// into an array-like whose size is a constant, for expansion statements
// (`template for (constexpr info m : l.items)`) until
// define_static_array (WP-479).
template <unsigned long N>
struct __kiln_info_array {
  info items[N == 0 ? 1 : N];
  constexpr unsigned long size() const { return N; }
  constexpr info operator[](unsigned long i) const { return items[i]; }
};
template <info R>
consteval auto __kiln_data_members() {
  constexpr unsigned long n = nonstatic_data_members_of(R, access_context::unchecked()).size();
  __kiln_info_array<n> l{};
  unsigned long i = 0;
  for (info m : nonstatic_data_members_of(R, access_context::unchecked())) l.items[i++] = m;
  return l;
}

// [meta.reflection.layout]
consteval member_offset offset_of(info r);
consteval decltype(sizeof 0) size_of(info r);
consteval decltype(sizeof 0) alignment_of(info r);
consteval decltype(sizeof 0) bit_size_of(info r);

// [meta.reflection.annotation] (P3394R4)
consteval bool is_annotation(info r);
consteval std::vector<info> annotations_of(info item);
consteval std::vector<info> annotations_of_with_type(info item, info type);
}  // namespace std::meta

// Defines a non-static data member `name` of type `type` in the class
// `cls`, which must be under definition (its member-specification is
// being parsed). Returns the reflection of the new member.
consteval std::meta::info __kiln_define_member(std::meta::info cls, std::meta::info type,
                                               const char* name);

namespace std::meta {
consteval bool name_is(info r, const char* s) {
  const char* n = name_of(r);
  for (; *n && *n == *s; ++n, ++s) {
  }
  return *n == *s;
}
}  // namespace std::meta

// [meta.reflection.define.aggregate] (WP-480; spec/reflection.md §6).
// `data_member_options` with a non-owning `name_type` (the Oven reads
// its flag, then the pointer and length of the string it holds; §6.1);
// `optional` is a stand-in without a standard library (libc++'s optional
// layout; `initializer_list` is the prelude's).
#if defined(_LIBCPP_VERSION) || defined(__GLIBCXX__)
#include <optional>
#else
namespace std {
template <class T>
class optional {
  union {
    char __null_state_;
    T __val_;
  };
  bool __engaged_ = false;

 public:
  constexpr optional() noexcept : __null_state_() {}
  template <class U = T>
    requires(!__is_same(__remove_cvref(U), optional))
  constexpr optional(U&& v) : __val_(static_cast<U&&>(v)), __engaged_(true) {}
  constexpr bool has_value() const noexcept { return __engaged_; }
  constexpr const T& operator*() const noexcept { return __val_; }
};
}  // namespace std
#endif

namespace std::meta {
struct data_member_options {
  struct name_type {
    bool __u8_ = false;
    const char8_t* __u8p_ = nullptr;
    decltype(sizeof 0) __u8n_ = 0;
    const char* __p_ = nullptr;
    decltype(sizeof 0) __n_ = 0;

    consteval name_type(const char* s) : __p_(s), __n_(__builtin_strlen(s)) {}
    consteval name_type(const char8_t* s) : __u8_(true), __u8p_(s) {
      while (s[__u8n_]) ++__u8n_;
    }
    consteval name_type(std::string_view s) : __p_(s.data()), __n_(s.size()) {}
    consteval name_type(std::u8string_view s) : __u8_(true), __u8p_(s.data()), __u8n_(s.size()) {}
  };
  std::optional<name_type> name;
  std::optional<int> alignment;
  std::optional<int> bit_width;
  bool no_unique_address = false;
};

consteval info data_member_spec(info type, data_member_options options);
consteval bool is_data_member_spec(info r);
template <class R = std::initializer_list<info>>
consteval info define_aggregate(info class_type, R&& mdescrs);
}  // namespace std::meta

// [meta.reflection.extract], [meta.reflection.result], [meta.define.static]
// (P3491R3; WP-479). extract and the reflect_* functions are intrinsics
// (spec/reflection-values.md); define_static_* are library functions over
// them, as libstdc++ defines them. define_static_array returns a span: the
// standard one when a standard library is in use, else a stand-in of the
// same layout.
#if defined(_LIBCPP_VERSION) || defined(__GLIBCXX__)
#include <span>
#else
namespace std {
template <class T>
class span {
  T* data_ = nullptr;
  decltype(sizeof 0) size_ = 0;

 public:
  constexpr span() noexcept = default;
  constexpr span(T* p, decltype(sizeof 0) n) noexcept : data_(p), size_(n) {}
  constexpr T* data() const noexcept { return data_; }
  constexpr decltype(sizeof 0) size() const noexcept { return size_; }
  constexpr bool empty() const noexcept { return size_ == 0; }
  constexpr T* begin() const noexcept { return data_; }
  constexpr T* end() const noexcept { return data_ + size_; }
  constexpr T& operator[](decltype(sizeof 0) i) const noexcept { return data_[i]; }
};
}  // namespace std
#endif

namespace std::meta {
template <class T>
consteval T extract(info r);
template <class T>
consteval info reflect_constant(const T& expr);
template <class T>
consteval info reflect_object(T& expr);
template <class T>
consteval info reflect_function(T& fn);
template <class R>
consteval info __kiln_reflect_constant_string(R&& r);
template <class R>
consteval info __kiln_reflect_constant_array(R&& r);

consteval bool __kiln_is_string_literal(const char* p);
consteval bool __kiln_is_string_literal(const wchar_t* p);
consteval bool __kiln_is_string_literal(const char8_t* p);
consteval bool __kiln_is_string_literal(const char16_t* p);
consteval bool __kiln_is_string_literal(const char32_t* p);

// range_value_t of a built-in array or a class with begin().
template <class T>
struct __kiln_unref {
  using type = T;
};
template <class T>
struct __kiln_unref<T&> {
  using type = T;
};
template <class T>
struct __kiln_unref<const T> : __kiln_unref<T> {};
template <class T>
struct __kiln_unref<const T&> : __kiln_unref<T> {};
template <class T>
T& __kiln_lval() noexcept;
template <class T, decltype(sizeof 0) N>
T* __kiln_begin(T (&a)[N]);
template <class R>
auto __kiln_begin(R& r) -> decltype(r.begin());
template <class R>
using __kiln_range_value_t =
    typename __kiln_unref<decltype(*__kiln_begin(__kiln_lval<R>()))>::type;

// The intrinsics read a built-in array (a string literal's terminator
// recognized) or a contiguous range by layout; a range with data() and
// size() is passed as a span of them, any other copied into one.
template <class F, class R>
consteval info __kiln_reflect_range(F f, R&& r) {
  using T = __kiln_range_value_t<R>;
  if constexpr (__is_array(typename __kiln_unref<R>::type)) {
    return f(r);
  } else if constexpr (requires { r.data(); r.size(); }) {
    return f(span<const T>(r.data(), r.size()));
  } else if constexpr (__is_array(T)) {
    return f(r);
  } else {
    decltype(sizeof 0) n = 0;
    for (auto&& x : r) {
      (void)x;
      ++n;
    }
    T* p = n ? new T[n] : nullptr;
    decltype(sizeof 0) i = 0;
    for (auto&& x : r) p[i++] = static_cast<T>(x);
    info v = f(span<const T>(p, n));
    delete[] p;
    return v;
  }
}
template <class R>
consteval info reflect_constant_string(R&& r) {
  return __kiln_reflect_range([](auto&& x) { return __kiln_reflect_constant_string(x); }, r);
}
template <class R>
consteval info reflect_constant_array(R&& r) {
  return __kiln_reflect_range([](auto&& x) { return __kiln_reflect_constant_array(x); }, r);
}
}  // namespace std::meta

namespace std {
consteval bool is_string_literal(const char* p) { return meta::__kiln_is_string_literal(p); }
consteval bool is_string_literal(const wchar_t* p) { return meta::__kiln_is_string_literal(p); }
consteval bool is_string_literal(const char8_t* p) { return meta::__kiln_is_string_literal(p); }
consteval bool is_string_literal(const char16_t* p) { return meta::__kiln_is_string_literal(p); }
consteval bool is_string_literal(const char32_t* p) { return meta::__kiln_is_string_literal(p); }

template <class R>
consteval const meta::__kiln_range_value_t<R>* define_static_string(R&& r) {
  using T = meta::__kiln_range_value_t<R>;
  return meta::extract<const T*>(meta::reflect_constant_string(r));
}

template <class R>
consteval span<const meta::__kiln_range_value_t<R>> define_static_array(R&& r) {
  using T = meta::__kiln_range_value_t<R>;
  decltype(sizeof 0) n = 0;
  for (auto&& x : r) {
    (void)x;
    ++n;
  }
  if (n == 0) return span<const T>();
  return span<const T>(meta::extract<const T*>(meta::reflect_constant_array(r)), n);
}

template <class T>
consteval const typename meta::__kiln_unref<T>::type* define_static_object(T&& t) {
  using U = typename meta::__kiln_unref<T>::type;
  if constexpr (__is_class(U) || __is_union(U)) {
    return __builtin_addressof(meta::extract<const U&>(meta::reflect_constant(t)));
  } else if constexpr (__is_array(U)) {
    return __builtin_addressof(meta::extract<const U&>(meta::reflect_constant_array(t)));
  } else {
    // A span, not `const U a[1]`: an array bound in this pattern would
    // cost every including TU an evaluation.
    return define_static_array(span<const U>(__builtin_addressof(t), 1)).data();
  }
}
}  // namespace std
