// [meta.reflection.names]/1-5 (P2996R13): has_identifier, identifier_of
// and u8identifier_of. A class, enumeration, enumerator, variable,
// function, member, namespace, namespace alias, type alias and template
// have the identifier they were declared with; an unnamed class with a
// typedef name for linkage has that name ([dcl.typedef]/9). The global
// namespace, specializations and types that are not a cv-unqualified
// class, enumeration or alias have none. (Operator, conversion and
// literal operator names are not yet reflectable by `^^`.) Checked
// against GCC 16.1's <meta>.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

namespace ns {
struct S {
  int mem;
  S();
  ~S();
  S& operator=(const S&);
  operator int() const;
  void fn();
  static int sm;
};
enum E { A, B };
enum class F : char { X };
typedef struct { int z; } Linkage;
int var = 0;
void free_fn(int);
template <class T> struct Tm {};
template <class T> T vt = T();
template <class T> using Al = Tm<T>;
using Alias = S;
}  // namespace ns
namespace na = ns;

static_assert(has_identifier(^^ns));
static_assert(identifier_of(^^ns) == "ns");
static_assert(identifier_of(^^ns::S) == "S");
static_assert(identifier_of(^^ns::S::mem) == "mem");
static_assert(identifier_of(^^ns::S::fn) == "fn");
static_assert(identifier_of(^^ns::S::sm) == "sm");
static_assert(identifier_of(^^ns::E) == "E");
static_assert(identifier_of(^^ns::F) == "F");
static_assert(identifier_of(^^ns::Linkage) == "Linkage");
static_assert(identifier_of(^^ns::var) == "var");
static_assert(identifier_of(^^ns::free_fn) == "free_fn");
static_assert(identifier_of(^^na) == "na");
static_assert(identifier_of(^^ns::B) == "B");
static_assert(identifier_of(^^ns::Alias) == "Alias");
static_assert(identifier_of(^^ns::Tm) == "Tm");
static_assert(identifier_of(^^ns::vt) == "vt");
static_assert(identifier_of(^^ns::Al) == "Al");

static_assert(u8identifier_of(^^ns::S) == u8"S");
static_assert(u8identifier_of(^^ns::S::mem) == u8"mem");
static_assert(identifier_of(^^ns::S).size() == 1);

static_assert(!has_identifier(^^int));
static_assert(!has_identifier(^^ns::S*));
static_assert(!has_identifier(^^const ns::S));
static_assert(!has_identifier(^^ns::Tm<int>));
static_assert(!has_identifier(^^ns::Al<int>));
static_assert(!has_identifier(^^::));

int main() {}
