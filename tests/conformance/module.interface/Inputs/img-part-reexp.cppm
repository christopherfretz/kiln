// A module that imports one module and re-exports another (spec/modules.md
// §11.11 stage S6); its image follows the replayed imports.
export module imgr;
import imgu;
export import imgp;
export namespace rr {
constexpr int seven() { return 7; }
} // namespace rr
