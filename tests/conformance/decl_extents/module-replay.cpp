// The declaration-extent table (spec/ast.md §19.6): a module unit
// replayed from its source records its rows by parsing, the same as
// those module-image.cpp's importer builds from the image (this file
// keeps that file's lines, so the two share their golden; the
// import is on the same line).
//@ module: extents_mod=Inputs/extents_mod.cppm
//@ decl-extents: module.extents
//@ args: -fkiln-decl-extents
//@ std: c++20
import extents_mod;
int use() { return geo::twice(2) + top(1); }
