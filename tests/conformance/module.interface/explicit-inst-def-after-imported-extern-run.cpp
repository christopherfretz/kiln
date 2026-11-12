// [temp.explicit]/10,11, [module.interface]: an importer's explicit
// instantiation definition of a function template specialization the
// imported module declared with `extern template` defines it in the
// importer's object (clang: `weak_odr`).
//@ expect: run
//@ std: c++26
//@ module: xidef=Inputs/extern-inst-then-def.cppm
import xidef;

template double twice<double>(double);

int main() { return twice(1.5) == 3.0 ? 0 : 1; }
