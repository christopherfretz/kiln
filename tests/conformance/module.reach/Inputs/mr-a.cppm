// Module mr.a for module.reach/missing-import.cpp: imported by mr.b but
// not re-exported, so its exported declarations are reachable, not
// visible, from mr.b's importers.
export module mr.a;
export int a_fn() { return 1; }
export struct AType {};
