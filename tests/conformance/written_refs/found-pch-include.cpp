// found-pch.cpp's prefix included as text (spec/ast.md §18.12): the
// same golden (the two files keep their code on the same lines).
//@ written-refs: found-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
#include "Inputs/found_pch.h"
//
//
//
void run() {
  fp::call(1);
  fp::S<int>{}.use(1);
}
