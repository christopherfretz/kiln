// [class.local], [class.nest], [module.interface]: local classes two deep
// in an exported inline function (a member class of a local class, and a
// local class of a local class's member function): their member functions
// are the module unit's, and the importer's copy of the function links.
//@ expect: run
//@ std: c++26
//@ module: lcnest=Inputs/local-class-nested.cppm
import lcnest;

int main() {
  if (lcnest_member(3) != 7)
    return 1;
  if (lcnest_inner(4) != 9)
    return 2;
  return 0;
}
