// [module.unit]/7, [basic.link]/11, [temp.friend]/1: the friend
// declaration an importer's instantiation of `fta::It<char>` makes
// redeclares `fta::peek`, attached to module `fta` like the template
// that declared it; it is not a declaration in the global module
// conflicting with the module's own (clang accepts).
//@ expect: run
//@ std: c++20
//@ module: fta=Inputs/friend-template-attach.cppm
import fta;
int main() {
  fta::It<char> i;
  return peek(i);
}
