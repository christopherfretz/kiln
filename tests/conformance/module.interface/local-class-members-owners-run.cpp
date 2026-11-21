// [class.local], [class.mfct]/1, [expr.prim.lambda.closure],
// [module.interface]: the member functions of local classes in an inline
// and a constexpr member function of an exported class and in a lambda of
// an exported inline variable are the module unit's; a function template
// both the module unit and the importer instantiate gives each its own
// linkonce_odr copy of its local class's member.
//@ expect: run
//@ std: c++26
//@ module: lcown=Inputs/local-class-owners.cppm
import lcown;

int main() {
  Host h{1};
  if (h.viaMember() != 101)
    return 1;
  if (h.viaConstexpr() != 201)
    return 2;
  if (viaLambda(1) != 301)
    return 3;
  if (twice(5) != 10)
    return 4;
  if (moduleSide() != 42)
    return 5;
  return 0;
}
