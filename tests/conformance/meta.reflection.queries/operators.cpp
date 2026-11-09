// [meta.reflection.queries], [meta.reflection.operators] P2996R13: operator_of
// maps an operator function or operator function template to its
// std::meta::operators enumerator; symbol_of / u8symbol_of map an enumerator to
// the operator's spelling ("new[]", "()", "co_await", "<=>") as a
// std::string_view / std::u8string_view of it. Operator functions are
// reached through members_of (^^ of an operator-function-id does not parse
// yet). Answers checked against GCC 16.1 (-freflection), whose operators
// numbering (op_new = 1, ..., op_comma) kiln_meta_queries.h copies.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

// The view equals the whole spelling: no terminator, no more characters.
template <class V, class C>
consteval bool same(V v, const C* s) {
  return v == V(s) && v.data()[v.size()] == 0;
}

static_assert(same(symbol_of(operators::op_new), "new"));
static_assert(same(symbol_of(operators::op_delete), "delete"));
static_assert(same(symbol_of(operators::op_array_new), "new[]"));
static_assert(same(symbol_of(operators::op_array_delete), "delete[]"));
static_assert(same(symbol_of(operators::op_co_await), "co_await"));
static_assert(same(symbol_of(operators::op_parentheses), "()"));
static_assert(same(symbol_of(operators::op_square_brackets), "[]"));
static_assert(same(symbol_of(operators::op_arrow), "->"));
static_assert(same(symbol_of(operators::op_arrow_star), "->*"));
static_assert(same(symbol_of(op_tilde), "~") && same(symbol_of(op_exclamation), "!"));
static_assert(same(symbol_of(op_plus), "+") && same(symbol_of(op_minus), "-"));
static_assert(same(symbol_of(op_star), "*") && same(symbol_of(op_slash), "/"));
static_assert(same(symbol_of(op_percent), "%") && same(symbol_of(op_caret), "^"));
static_assert(same(symbol_of(op_ampersand), "&") && same(symbol_of(op_equals), "="));
static_assert(same(symbol_of(op_pipe), "|") && same(symbol_of(op_plus_equals), "+="));
static_assert(same(symbol_of(op_minus_equals), "-=") && same(symbol_of(op_star_equals), "*="));
static_assert(same(symbol_of(op_slash_equals), "/=") && same(symbol_of(op_percent_equals), "%="));
static_assert(same(symbol_of(op_caret_equals), "^=") && same(symbol_of(op_ampersand_equals), "&="));
static_assert(same(symbol_of(op_pipe_equals), "|=") && same(symbol_of(op_equals_equals), "=="));
static_assert(same(symbol_of(op_exclamation_equals), "!=") && same(symbol_of(op_less), "<"));
static_assert(same(symbol_of(op_greater), ">") && same(symbol_of(op_less_equals), "<="));
static_assert(same(symbol_of(op_greater_equals), ">=") && same(symbol_of(op_spaceship), "<=>"));
static_assert(same(symbol_of(op_ampersand_ampersand), "&&") && same(symbol_of(op_pipe_pipe), "||"));
static_assert(same(symbol_of(op_less_less), "<<") && same(symbol_of(op_greater_greater), ">>"));
static_assert(same(symbol_of(op_less_less_equals), "<<=") && same(symbol_of(op_greater_greater_equals), ">>="));
static_assert(same(symbol_of(op_plus_plus), "++") && same(symbol_of(op_minus_minus), "--"));
static_assert(same(symbol_of(op_comma), ","));
static_assert(same(u8symbol_of(op_spaceship), u8"<=>") && same(u8symbol_of(op_array_new), u8"new[]"));

struct S {
  bool operator==(const S&) const;          // 0
  S& operator=(const S&);                    // 1
  int operator()(int) const;                 // 2
  int operator[](int) const;                 // 3
  S* operator->();                           // 4
  S& operator++();                           // 5
  int operator<=>(int) const;                // 6
  template <class T> S& operator<<(T);       // 7
  static void* operator new(decltype(sizeof 0));    // 8
  static void operator delete[](void*);      // 9
  operator int() const;                      // 10
};
consteval info m(unsigned long i) { return members_of(^^S, access_context::unchecked())[i]; }

static_assert(operator_of(m(0)) == operators::op_equals_equals);
static_assert(operator_of(m(1)) == op_equals && operator_of(m(2)) == op_parentheses);
static_assert(operator_of(m(3)) == op_square_brackets && operator_of(m(4)) == op_arrow);
static_assert(operator_of(m(5)) == op_plus_plus && operator_of(m(6)) == op_spaceship);
static_assert(operator_of(m(7)) == op_less_less);  // an operator function template
static_assert(operator_of(m(8)) == op_new && operator_of(m(9)) == op_array_delete);
static_assert(same(symbol_of(operator_of(m(6))), "<=>"));
