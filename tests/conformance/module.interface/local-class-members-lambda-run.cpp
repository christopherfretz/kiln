// [class.local], [expr.prim.lambda.closure], [module.interface]: a lambda
// and a generic lambda in a member function of a local class of an
// exported inline function: the member and the call operators it uses
// are emitted by the module unit, and the importer's copy links.
//@ expect: run
//@ std: c++26
//@ module: lclam=Inputs/local-class-lambda.cppm
import lclam;

int main() {
  if (lclam_call(4) != 14)
    return 1;
  if (lclam_generic(1) != 7)
    return 2;
  return 0;
}
