// Operators and found sets (spec/ast.md §18.12): an operator-function-id
// written as a call is a name like any other, with a found set open to
// argument-dependent lookup ([over.match.oper]/1 does not apply to it).
// A dependent operator expression is a DEPENDENT entry with the found set
// of its definition context: the non-member candidates of unqualified
// lookup and the ADL flag ([over.match.oper]/3.2, §18.13.6).
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
