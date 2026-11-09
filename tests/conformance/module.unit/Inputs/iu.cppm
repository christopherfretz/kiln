// Primary interface of module `iu` (two-step-impl-unit-run.cpp): it
// declares what its implementation unit defines.
export module iu;
export int iu_value();
export struct Acc {
  int total = 0;
  void add(int v);
};
int iu_base = 30;
