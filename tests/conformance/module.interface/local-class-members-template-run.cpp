// [class.local], [temp.inst], [dcl.inline], [module.interface]: a local
// class of an exported inline function template and of a member function
// template, instantiated by the importer (its members are the importer's
// linkonce_odr copies), and an explicitly inline member of a local class
// of an exported inline function.
//@ expect: run
//@ std: c++26
//@ module: lctmpl=Inputs/local-class-template.cppm
import lctmpl;

int main() {
  if (lctmpl_twice(6) != 12)
    return 1;
  if (lctmpl_twice(1.5) != 3.0)
    return 2;
  if (Box<int>{3}.scaled(2L) != 6L)
    return 3;
  if (lctmpl_inline(9) != 9)
    return 4;
  return 0;
}
