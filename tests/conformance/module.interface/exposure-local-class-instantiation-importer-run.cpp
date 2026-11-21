// [basic.link]/14.1, /15.5, [basic.link]/18 (WP-1846): an importer calls
// functions of a module that use specializations of its function templates
// whose local classes and closures name TU-local entities: in the body of
// a function that is not inline (ignored, also a specialization returning
// such a lambda or class), and in inline functions where the deduced
// return type names no such class or the member is not inline; each runs
// in the importer.
//@ expect: run
//@ std: c++20
//@ module: xlci=Inputs/exposure-local-class-instantiation.cppm
import xlci;

int main() {
  // 4 + 3, 5, 6.
  return ignored(3) == 7 && inline_not_returned(4) == 5 && inline_not_inline(5) == 6 ? 0 : 1;
}
