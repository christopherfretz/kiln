// [temp.deduct.conv]/1, [temp.deduct.general]/8, [over.match.copy]/1.2:
// inside an enclosing template, a conversion function template whose
// deduction against the target type genuinely fails (its constraint is not
// satisfied for the deduced `U`) still leaves no viable conversion: the
// non-dependent one is diagnosed at the definition, the dependent one at
// the instantiation, and neither borrows the enclosing instantiation's
// argument for `U` (WP-563). The well-formed conversion next to them is
// accepted in both instantiations.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:12 error "no viable conversion from 'gate' to 'stranger'"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'gate' to 'const stranger &' for 1st argument"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'gate' to 'stranger &&' for 1st argument"
//@ diag: 26:3 note "candidate template ignored: requirement 'std::is_base_of_v<base, stranger>' was not satisfied [with U = stranger]"
//@ diag: 44:8 error "no viable conversion from 'gate' to 'stranger'"
//@ diag: 48:73 note "in instantiation of function template specialization 'dependent<stranger>' requested here"
//@ diag: 23:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'gate' to 'const stranger &' for 1st argument"
//@ diag: 23:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'gate' to 'stranger &&' for 1st argument"
//@ diag: 26:3 note "candidate template ignored: requirement 'std::is_base_of_v<base, stranger>' was not satisfied [with U = stranger]"
#include <type_traits>

struct base { int id = 0; };
struct stranger { int id = 0; };
struct gate {
  template <class U, std::enable_if_t<std::is_base_of_v<base, U>, int> = 0>
  operator U() const & { return U(); }
};

template <class Self> int fine() {
  gate g;
  base b = g;
  return b.id;
}

template <class Self> int non_dependent() {
  gate g;
  stranger s = g;
  return s.id;
}

template <class Self> int dependent() {
  gate g;
  base b = g;
  Self s = g;
  return b.id + s.id;
}

int use() { return fine<stranger>() + fine<int>() + dependent<base>() + dependent<stranger>(); }
