// Dependent operators in a requirement and in a deduction context
// (spec/ast.md §18.13.6): the operator of a simple-requirement
// ([expr.prim.req.simple]/1) has its entry and, for each satisfaction
// check that substitutes it ([temp.constr.atomic]/3), a row keyed by the
// concept's specialization. An operator in a return type that fails
// substitution ([temp.deduct.general]/8, a SFINAE context) records no row
// for the failed candidate; the specialization that succeeds has its row.
//@ written-refs: operators-dependent-sfinae.refs
namespace lib {
struct S {
  int v;
};
S operator-(S, S);
struct N {};
} // namespace lib

template <class T>
concept Sub = requires(T a) { a - a; };
template <Sub T> int con(T) { return 1; }
template <class T> int con(T) { return 0; }

template <class T> auto mul(T t) -> decltype(t * t) { return t * t; }
template <class T> int mul(...) { return 0; }

int run() {
  return con(lib::S{}) + con(lib::N{}) + con(1) + mul<lib::N>(lib::N{}) +
         mul(2);
}
