// Module for ../module-enum.cpp.
export module E;
namespace P {
  export enum class En { a };
  export struct Cl {};
  export int h(En) { return 1; }
  inline namespace I {
    export int k(Cl) { return 2; }
  }
}
