// Operators and found sets (spec/ast.md §18.12): an operator-function-id
// written as a call is a name like any other, with a found set open to
// argument-dependent lookup ([over.match.oper]/1 does not apply to it).
// An operator expression has no written name and no entry (§18.4); its
// definition-context lookup set is kept by the template machinery
// (spec/ast.md §8.5), not by the table.
//@ written-refs: found-operator.refs
namespace lib {
struct X {};
X operator+(X, X);
} // namespace lib
lib::X operator-(lib::X, lib::X);
template <class T> T call_form(T t) { return operator-(t, t); }
template <class T> T expr_form(T t) { return t + t; }
void run() {
  call_form(lib::X{});
  expr_form(lib::X{});
}
