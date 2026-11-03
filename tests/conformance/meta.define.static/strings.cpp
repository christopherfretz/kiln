// [meta.string.literal], [meta.define.static] (P3491R3),
// spec/reflection-values.md §4: is_string_literal, reflect_constant_string
// and define_static_string. The result is a template parameter object of
// type const CharT[N + 1] holding the range's characters and a
// terminator; a string literal argument keeps its own terminator (no
// double null), any other range gets one. Equal contents give the same
// object, so the pointer is a valid constant template argument.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

// is_string_literal: a string literal or a subobject of one.
static_assert(std::is_string_literal("abc"));
static_assert(std::is_string_literal("abc" + 2));
static_assert(std::is_string_literal(L"abc"));
static_assert(std::is_string_literal(u8"abc"));
static_assert(std::is_string_literal(u"abc"));
static_assert(std::is_string_literal(U"abc"));
constexpr char arr[] = "abc";
static_assert(!std::is_string_literal(arr));
constexpr const char* lit = "xyz";
static_assert(std::is_string_literal(lit));
static_assert(!std::is_string_literal(static_cast<const char*>(nullptr)));
static_assert(!std::is_string_literal(std::define_static_string("abc")));

// define_static_string: the characters and one terminator.
constexpr const char* s = std::define_static_string("hello");
static_assert(s[0] == 'h' && s[4] == 'o' && s[5] == '\0');
static_assert(std::define_static_string("hello") == s);
static_assert(std::define_static_string("hell") != s);
// arr is not a string literal: its terminator is one of its characters.
static_assert(std::define_static_string(arr)[3] == '\0');
static_assert(std::define_static_string(arr)[4] == '\0');
static_assert(std::define_static_string(arr) != std::define_static_string("abc"));
constexpr char cs[] = {'a', 'b'};
static_assert(std::define_static_string(cs)[0] == 'a');
static_assert(std::define_static_string(cs)[2] == '\0');
static_assert(std::define_static_string(cs) == std::define_static_string("ab"));
static_assert(std::define_static_string(std::string_view("hello")) == s);
static_assert(std::define_static_string(std::string_view("hello", 3)) ==
              std::define_static_string("hel"));
static_assert(*std::define_static_string("") == '\0');

// Other character types.
static_assert(std::define_static_string(u8"hé")[2] == char8_t(0xA9));
static_assert(std::define_static_string(L"wide")[4] == L'\0');
static_assert(std::define_static_string(u"u16")[1] == u'1');
static_assert(std::define_static_string(U"u32")[3] == U'\0');
static_assert(std::define_static_string(L"w") != nullptr);

// reflect_constant_string: content-interned reflections of the object.
static_assert(reflect_constant_string("ab") == reflect_constant_string("ab"));
static_assert(reflect_constant_string("ab") != reflect_constant_string("abc"));
static_assert(reflect_constant_string(cs) == reflect_constant_string("ab"));
static_assert(reflect_constant_string("ab") != reflect_constant_string(u8"ab"));
static_assert(extract<const char*>(reflect_constant_string("ab")) ==
              std::define_static_string("ab"));
static_assert(extract<const char (&)[3]>(reflect_constant_string("ab"))[1] == 'b');

// P3491R3 §3.1 and §3.5.1: as a constant template argument.
template <const char* P>
struct C {};
const char msg[] = "strongly in favor";  // just an idea..
C<msg> c1;                               // ok
C<std::define_static_string("yay")> c3;  // ok
// Template-argument-equivalent by content.
static_assert(__is_same(C<std::define_static_string("dup")>, C<std::define_static_string("dup")>));
static_assert(!__is_same(C<std::define_static_string("dedup")>, C<std::define_static_string("dup")>));
static_assert(!__is_same(C<std::define_static_string("dup")>, C<msg>));

// P3491R3 §3.5.5: the single-pointer source_location, after Richard Smith
// (a pointer is not a range, so the file name is passed as a string_view).
class source_location {
  struct impl {
    char const* filename;
    int line;
  };
  impl const* p_;
  constexpr source_location(impl const* p) : p_(p) {}

 public:
  static consteval auto current(char const* file = __builtin_FILE(),
                                int line = __builtin_LINE()) noexcept -> source_location {
    // first, we canonicalize the file
    impl data = {.filename = std::define_static_string(std::string_view(file)), .line = line};
    // then we canonicalize the data
    impl const* p = std::define_static_object(data);
    // and now we have an external linkage object mangled with this location
    return source_location{p};
  }
  constexpr int line() const { return p_->line; }
  constexpr const char* file() const { return p_->filename; }
  constexpr bool same(source_location o) const { return p_ == o.p_; }
};
constexpr auto loc1 = source_location::current();
constexpr auto loc2 = source_location::current();
constexpr auto loc3 = source_location::current("f.cpp", 1);
static_assert(loc1.line() == 98 && loc2.line() == 99);
static_assert(!loc1.same(loc2));
static_assert(loc3.same(source_location::current("f.cpp", 1)));
static_assert(loc1.file() == loc2.file());
static_assert(loc3.file()[0] == 'f' && loc3.file()[5] == '\0');
