// [temp.inst]/3, [dcl.attr.grammar]/5: template-redecl-ir.cpp at -O1
// (the front end's IR, before LLVM's passes): the specializations'
// argument attributes are the same. spec/sema.md §4.5 "Parameter
// attributes" rule 5, WP-3407.
//@ expect: ir
//@ std: c++20
//@ args: -O1 -Xclang -disable-llvm-passes
#include "Inputs/template-redecl.h"
