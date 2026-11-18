// __builtin_has_attribute on a type-id operand asks the attributes written
// in its decl-specifier-seq first (GCC keeps them on the type it builds;
// kiln, as clang, drops them from the type), then the type. An `aligned`
// with an argument compares the requested alignment. A function or variable
// declared with visibility "internal" answers "internal", not "hidden"
// (both emit hidden). Found running GCC's C++ testsuite. WP-1667.
//@ std: c++20
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
#define ATTR(...) __attribute__((__VA_ARGS__))

static_assert(__builtin_has_attribute(ATTR(aligned) char, aligned));
static_assert(__builtin_has_attribute(ATTR(aligned(4)) int, __aligned__));
static_assert(__builtin_has_attribute(int ATTR(aligned(8)), aligned(8)));
static_assert(__builtin_has_attribute(int ATTR(aligned(8)), aligned(1 + 7)));
static_assert(!__builtin_has_attribute(int ATTR(aligned(8)), aligned(4)));
static_assert(__builtin_has_attribute(int ATTR(aligned(2), aligned(4)), aligned(2)));
static_assert(__builtin_has_attribute(int ATTR(aligned(2), aligned(4)), aligned(4)));
static_assert(!__builtin_has_attribute(int, aligned));
static_assert(__builtin_has_attribute(ATTR(may_alias) int, may_alias));
static_assert(!__builtin_has_attribute(ATTR(may_alias) int, aligned));
static_assert(!__builtin_has_attribute(int, may_alias));

ATTR(visibility("internal")) void internal_fn();
ATTR(visibility("hidden")) void hidden_fn();
ATTR(visibility("internal")) int internal_var;
static_assert(__builtin_has_attribute(internal_fn, visibility));
static_assert(__builtin_has_attribute(internal_fn, visibility("internal")));
static_assert(!__builtin_has_attribute(internal_fn, visibility("hidden")));
static_assert(__builtin_has_attribute(hidden_fn, visibility("hidden")));
static_assert(!__builtin_has_attribute(hidden_fn, visibility("internal")));
static_assert(__builtin_has_attribute(internal_var, visibility("internal")));
static_assert(!__builtin_has_attribute(internal_var, visibility("default")));
