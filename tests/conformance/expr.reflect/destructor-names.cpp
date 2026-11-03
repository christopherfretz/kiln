// [expr.reflect]/7 (P2996R13), [class.dtor]/1: `^^S::~S` reflects the
// destructor of S, declared or implicit; `^^T::~T` in a template reflects
// the destructor of the class T is instantiated with. A destructor has no
// identifier ([meta.reflection.names]) and no type ([meta.reflection.queries]
// *has-type* excludes constructors and destructors).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
using std::meta::info;
struct S { ~S(); };
struct Implicit { int x; };
namespace ns { struct T { virtual ~T(); }; }

static_assert(std::meta::is_destructor(^^S::~S));
static_assert(std::meta::is_function(^^S::~S));
static_assert(!std::meta::has_identifier(^^S::~S));
static_assert(std::meta::parent_of(^^S::~S) == ^^S);
static_assert(std::meta::is_noexcept(^^S::~S) && !std::meta::is_virtual(^^S::~S));
static_assert(std::meta::is_virtual(^^ns::T::~T));
static_assert(std::meta::is_destructor(^^Implicit::~Implicit));
static_assert(std::meta::parent_of(^^Implicit::~Implicit) == ^^Implicit);
static_assert(std::meta::is_destructor(^^ns::T::~T));
static_assert(^^S::~S != ^^ns::T::~T);

template <class T> consteval info r_dtor() { return ^^T::~T; }
static_assert(r_dtor<S>() == ^^S::~S);
static_assert(r_dtor<ns::T>() == ^^ns::T::~T);
static_assert(std::meta::is_destructor(r_dtor<Implicit>()));
