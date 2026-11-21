// [basic.link]/15.1, [basic.link]/18: an importer calls the functions of a
// module whose bodies hold local classes and closures naming TU-local
// entities (a constexpr and an inline member function, a base and member
// type, a called lambda), and the deduced return type naming a local class
// whose member function that is not inline calls a TU-local function; each
// runs in the importer.
//@ expect: run
//@ std: c++20
//@ module: xlcl=Inputs/exposure-local-class.cppm
import xlcl;

int main() {
  auto l = member_body();
  // 7, 2, 3, 5, 2, 4.
  return constexpr_member() == 7 && inline_member() == 2 && declarations() == 3 &&
                 called_lambda(4) == 5 && not_returned() == 2 && l.g(3) == 4
             ? 0
             : 1;
}
