// [meta.reflection.queries] P2996R13, [temp.pre]: the template-kind
// predicates (is_template, is_function_template, is_variable_template,
// is_class_template, is_alias_template, is_conversion_function_template,
// is_operator_function_template, is_literal_operator_template,
// is_constructor_template, is_concept) and has_template_arguments: true
// for a specialization of a function, variable, class or alias template,
// false for a template itself, a type alias that names a specialization,
// and a member of a specialization. Answers checked against GCC 16.1
// (-freflection). Member templates of a class are reached through
// __kiln_member_at (^^ of an operator-function-id does not parse yet).
// template_of / template_arguments_of are WP-482's.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

template <class T> struct CT { T t; };
template <class T> using AT = CT<T>;
template <class T> using AI = int;
template <class T> T VT = T();
template <class T> void FT(T) {}
template <class T> concept C = true;
using CA = CT<int>;
struct S {
  template <class T> S(T);                 // 0
  template <class T> operator T*();        // 1
  template <class T> bool operator<(T);    // 2
  template <class T> void mt();            // 3
  void nf();                               // 4
};
consteval info m(unsigned long i) { return __kiln_member_at(^^S, i); }
void plain();

static_assert(is_template(^^CT) && is_class_template(^^CT) && !is_alias_template(^^CT));
static_assert(is_template(^^AT) && is_alias_template(^^AT) && !is_class_template(^^AT));
static_assert(is_template(^^VT) && is_variable_template(^^VT) && !is_variable(^^VT));
static_assert(is_template(^^FT) && is_function_template(^^FT) && !is_function(^^FT));
static_assert(is_template(^^C) && is_concept(^^C) && !is_class_template(^^C));
static_assert(!is_template(^^S) && !is_template(^^CT<int>) && !is_template(^^plain));
static_assert(!is_concept(^^CT) && !is_template(info{}));

static_assert(is_constructor_template(m(0)) && is_function_template(m(0)) && !is_constructor(m(0)));
static_assert(is_conversion_function_template(m(1)) && !is_conversion_function(m(1)));
static_assert(is_operator_function_template(m(2)) && !is_operator_function(m(2)));
static_assert(!is_operator_function_template(m(1)) && !is_conversion_function_template(m(2)));
static_assert(is_function_template(m(3)) && !is_constructor_template(m(3)));
static_assert(!is_function_template(m(4)) && !is_literal_operator_template(m(3)));
static_assert(!is_literal_operator_template(^^FT));

static_assert(has_template_arguments(^^CT<int>) && has_template_arguments(^^AT<int>));
static_assert(has_template_arguments(^^AI<int>));  // the alias specialization, not int
static_assert(has_template_arguments(^^VT<int>) && has_template_arguments(^^FT<int>));
static_assert(!has_template_arguments(^^CT) && !has_template_arguments(^^S));
static_assert(!has_template_arguments(^^CA) && !has_template_arguments(^^int));
static_assert(!has_template_arguments(^^CT<int>::t) && !has_template_arguments(^^plain));
static_assert(!has_template_arguments(info{}));
