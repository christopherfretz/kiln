// Module for ../module-enum-overload.cpp: an unscoped enumeration and a
// function of its namespace, both attached to E2.
export module E2;
namespace P {
  export enum Un { a };
  export int h(Un) { return 1; }
}
