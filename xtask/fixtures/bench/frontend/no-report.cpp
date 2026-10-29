// A `frontend` entry for which the fake kiln (`xtask/fixtures/fake-kiln/kiln`)
// prints no instantiation report: measuring it with kiln fails
// (spec/sema-templates.md §11.4).
//@ std: c++20
template <class T> struct W { T t; };
W<int> w;
