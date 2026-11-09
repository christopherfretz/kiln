// [meta.reflection.queries] P2996R13 with P3096R12 (function parameter
// reflection): is_function_parameter, is_explicit_object_parameter,
// has_default_argument, is_vararg_function, return_type_of and
// variable_of. A parameter reflection comes from parameters_of; a
// variable that is a parameter, named inside its function, is a
// variable, not a function parameter reflection; the predicates are
// false for everything else; return_type_of takes a function or a
// function type (the declared return type, deduced once the function is
// defined); is_vararg_function takes either too. Answers checked
// against GCC 16.1 (-freflection).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

void g(int x, int y = 2, ...);
void k(int);
void k(int = 4);
auto deduced() { return 1L; }
struct S {
  void self(this S& s, int);
  int mf(char) const;
  S& operator=(const S&);
};
using F = double(int, ...);

static_assert(is_vararg_function(^^g) && !is_vararg_function(^^k));
static_assert(is_vararg_function(^^F) && !is_vararg_function(^^void(int)));
static_assert(!is_vararg_function(^^int) && !is_vararg_function(info{}));

static_assert(return_type_of(^^g) == (^^void) && return_type_of(^^S::mf) == ^^int);
static_assert(return_type_of(^^F) == (^^double) && return_type_of(^^deduced) == ^^long);
static_assert(return_type_of(members_of(^^S, access_context::unchecked())[2]) == ^^S&);

static_assert(!is_function_parameter(^^g) && !is_function_parameter(^^int));
static_assert(!is_explicit_object_parameter(^^g) && !has_default_argument(^^g));
static_assert(!has_default_argument(^^int) && !is_explicit_object_parameter(info{}));

void body(int p, int q = 1) {
  static_assert(is_variable(^^p) && !is_function_parameter(^^p));
  static_assert(!has_default_argument(^^q) && type_of(^^p) == ^^int);
  static_assert(parent_of(^^p) == (^^body) && has_automatic_storage_duration(^^p));
}

static_assert(is_function_parameter(parameters_of(^^g)[0]));
static_assert(!is_variable(parameters_of(^^g)[0]));
static_assert(!has_default_argument(parameters_of(^^g)[0]));
static_assert(has_default_argument(parameters_of(^^g)[1]));
static_assert(has_default_argument(parameters_of(^^k)[0]));  // a later redeclaration's
static_assert(type_of(parameters_of(^^g)[1]) == ^^int);
static_assert(parent_of(parameters_of(^^g)[0]) == ^^g);
static_assert(!has_linkage(parameters_of(^^g)[0]));
static_assert(is_explicit_object_parameter(parameters_of(^^S::self)[0]));
static_assert(!is_explicit_object_parameter(parameters_of(^^S::self)[1]));
static_assert(return_type_of(^^S::self) == ^^void);

// P3096R12's variable_of example, adapted to a defined function.
void f(int);
void f(int x) {
  constexpr auto rp = parameters_of(^^f)[0];
  static_assert(variable_of(rp) == ^^x);
  static_assert(is_function_parameter(rp) && !is_variable(rp));
}
