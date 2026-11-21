// [basic.link]/14.5, /17, [basic.link]/18: an importer uses the inline
// functions and variables of a module whose overload sets hold TU-local
// functions only where the sets are not named in an exposure (an operator
// expression, bodies and initializers that are not inline, specializations
// whose calls find no TU-local function); each is usable, and runs, in
// the importer.
//@ expect: run
//@ std: c++20
//@ module: xovs=Inputs/exposure-overload-set.cppm
import xovs;

int main() {
  // 3, 2 + 3 + 1 + 4 = 10, 2 + 1 = 3, 5, 2.
  return operator_expression().v == 3 && not_inline_body() == 10 && variable == 3 &&
                 instantiation() == 5 && non_inline_template() == 2 && N::adl(N::A{}) == 3
             ? 0
             : 1;
}
