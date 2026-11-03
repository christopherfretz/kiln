// [meta.reflection.queries] P2996R13: the entity-kind predicates
// (is_variable, is_type, is_namespace, is_type_alias, is_namespace_alias,
// is_function, is_enumerator, is_structured_binding, is_value, is_object,
// is_function_parameter, is_data_member_spec, is_annotation). Each is
// false, never an error, for a reflection of any other kind, the null
// reflection included. is_type_alias / is_namespace_alias see the alias
// ^^ kept ([expr.reflect]/6); is_type is true for both a type and a type
// alias, is_namespace for a namespace and a namespace alias. Answers
// checked against GCC 16.1 (-freflection). is_data_member_spec is
// always false until data_member_spec exists (WP-480), and is_annotation
// until annotations do (WP-481).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

using A = int;
typedef long TL;
namespace N { int nv; }
namespace NA = N;
int v;
void fn(int);
struct S {
  int m;
  static int sm;
  void mf();
  using MA = char;
  enum E { e1 };
};
enum class EC { a };
struct P { int x; int y; };
auto [b1, b2] = P{1, 2};
template <class T> struct CT {};

static_assert(is_variable(^^v) && is_variable(^^S::sm) && is_variable(^^N::nv));
static_assert(!is_variable(^^S::m) && !is_variable(^^fn) && !is_variable(^^int));
static_assert(!is_variable(^^b1));

static_assert(is_type(^^int) && is_type(^^S) && is_type(^^A) && is_type(^^TL));
static_assert(is_type(^^S::MA) && is_type(^^const int) && is_type(^^void()));
static_assert(!is_type(^^v) && !is_type(^^N) && !is_type(^^CT));

static_assert(is_type_alias(^^A) && is_type_alias(^^TL) && is_type_alias(^^S::MA));
static_assert(!is_type_alias(^^int) && !is_type_alias(^^S) && !is_type_alias(^^const A));

static_assert(is_namespace(^^N) && is_namespace(^^NA) && is_namespace(^^::));
static_assert(!is_namespace(^^S) && !is_namespace(^^v));
static_assert(is_namespace_alias(^^NA) && !is_namespace_alias(^^N));

static_assert(is_function(^^fn) && is_function(^^S::mf));
static_assert(!is_function(^^void()) && !is_function(^^v) && !is_function(^^CT));

static_assert(is_enumerator(^^S::e1) && is_enumerator(^^EC::a));
static_assert(!is_enumerator(^^EC) && !is_enumerator(^^v));

static_assert(is_structured_binding(^^b1) && is_structured_binding(^^b2));
static_assert(!is_structured_binding(^^v) && !is_structured_binding(^^P::x));

static_assert(!is_value(^^v) && !is_object(^^v) && !is_value(^^EC::a));
static_assert(!is_function_parameter(^^v) && !is_function_parameter(^^fn));

static_assert(!is_data_member_spec(^^S::m) && !is_annotation(^^S));

constexpr info null{};
static_assert(!is_variable(null) && !is_type(null) && !is_namespace(null));
static_assert(!is_function(null) && !is_enumerator(null) && !is_value(null));
static_assert(!is_object(null) && !is_type_alias(null) && !is_structured_binding(null));
