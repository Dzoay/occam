# Open CASCADE names FreeCAD uses

FreeCAD 1.1.4, commit `4fd3bf320d9566a27e60069fc8387448aaa3a094`. Made by `tools/census/freecad_occt_usage.py`, a script developed on synthetic
fixtures, so that only this table was read. Input to `census.parity`; class-level detail only.

## Method and limits

- Scanned 4735 C and C++ source files. Comments and string literals are not counted. `#include` lines count as includes, not uses.
- A name counts as OCCT only if the pinned OCCT wheel exports it, which includes enumerators. A name in a disabled `#if` branch counts, since nothing is parsed.
- Static calls count `Class::method(` where the class is named at the call site, not calls through an object.
- Ranking and totals count FreeCAD's own modules. `tests` and bundled `3rdParty` code appear after the semicolon and are not ranked.

## Names by use (617 names)

| Name | Uses | Includes | Per module |
|---|---:|---:|---|
| `TopoDS_Shape` | 2183 | 107 | Part 1123, TechDraw 529, CAM 144, PartDesign 105, Import 93, Measure 66, Fem 47, MeshPart 29, Surface 20, Sketcher 19, Inspection 5, ReverseEngineering 2, Mesh 1; 3rdParty/salomesmesh 1157, tests 65 |
| `gp_Pnt` | 1409 | 96 | Part 604, TechDraw 316, CAM 117, PartDesign 99, Sketcher 68, Import 57, Fem 48, Measure 36, ReverseEngineering 26, MeshPart 19, Inspection 7, Robot 7, Surface 5; 3rdParty/salomesmesh 559, tests 246 |
| `Standard_Failure` | 1015 | 65 | Part 846, PartDesign 44, TechDraw 37, Surface 33, Fem 26, Import 9, Sketcher 9, Measure 4, CAM 3, MeshPart 2, ReverseEngineering 2; 3rdParty/salomesmesh 30 |
| `Precision` | 943 | 112 | Part 350, Sketcher 249, PartDesign 122, TechDraw 75, CAM 63, Fem 33, Base 15, Measure 11, Assembly 7, Import 7, Surface 7, ReverseEngineering 3, MeshPart 1; 3rdParty/salomesmesh 94 |
| `TopoDS` | 888 | 217 | Part 522, TechDraw 108, PartDesign 73, Measure 44, CAM 34, Fem 32, Sketcher 25, Surface 20, Import 16, MeshPart 8, Assembly 4, Inspection 1, Robot 1; 3rdParty/salomesmesh 478, tests 10 |
| `TopoDS_Edge` | 723 | 121 | TechDraw 410, Part 182, MeshPart 32, Measure 18, PartDesign 17, Sketcher 16, Import 15, CAM 13, Fem 12, Surface 5, Robot 2, Mesh 1; 3rdParty/salomesmesh 510, tests 12 |
| `gp_Dir` | 612 | 52 | Part 274, TechDraw 136, PartDesign 96, Sketcher 29, CAM 23, Import 21, Measure 16, Fem 12, MeshPart 3, ReverseEngineering 2; 3rdParty/salomesmesh 48, tests 6 |
| `gp_Vec` | 538 | 34 | Part 241, TechDraw 142, PartDesign 48, Measure 44, Sketcher 26, Fem 19, Import 8, ReverseEngineering 4, CAM 2, Inspection 2, Surface 2; 3rdParty/salomesmesh 262, tests 15 |
| `TopAbs_EDGE` | 432 | 0 | Part 256, TechDraw 52, PartDesign 41, Measure 17, CAM 16, Sketcher 15, Surface 12, Import 9, MeshPart 8, Fem 5, Inspection 1; 3rdParty/salomesmesh 261, tests 39 |
| `TopoDS_Face` | 399 | 89 | Part 223, TechDraw 85, PartDesign 39, Fem 21, Measure 10, MeshPart 9, Sketcher 6, Surface 3, CAM 2, Assembly 1; 3rdParty/salomesmesh 306, tests 6 |
| `TopoDS_Wire` | 375 | 42 | Part 194, TechDraw 119, CAM 14, Import 11, PartDesign 10, MeshPart 9, Surface 8, Sketcher 5, Measure 3, ReverseEngineering 2; 3rdParty/salomesmesh 10, tests 8 |
| `TopAbs_FACE` | 326 | 0 | Part 200, PartDesign 41, TechDraw 22, Import 17, CAM 13, Surface 10, Measure 8, Fem 7, Sketcher 5, MeshPart 2, Inspection 1; 3rdParty/salomesmesh 240, tests 48 |
| `BRep_Tool` | 325 | 115 | Part 176, TechDraw 74, CAM 23, MeshPart 11, PartDesign 9, Import 8, Sketcher 8, Measure 7, Fem 5, Surface 4; 3rdParty/salomesmesh 277, tests 2 |
| `Geom_Curve` | 247 | 28 | Part 205, TechDraw 11, Sketcher 9, CAM 6, MeshPart 5, Measure 3, Surface 3, Fem 2, Import 2, PartDesign 1; 3rdParty/salomesmesh 58 |
| `TopExp_Explorer` | 241 | 136 | Part 114, TechDraw 53, CAM 30, Import 16, PartDesign 15, MeshPart 6, Sketcher 3, Surface 3, Inspection 1; 3rdParty/salomesmesh 165, tests 11 |
| `BRepAdaptor_Curve` | 227 | 83 | TechDraw 76, Part 57, Import 24, Measure 23, PartDesign 16, Sketcher 10, CAM 7, Fem 5, MeshPart 3, Surface 3, Assembly 2, Robot 1; 3rdParty/salomesmesh 23, tests 2 |
| `Geom_TrimmedCurve` | 209 | 22 | Part 166, Sketcher 26, TechDraw 11, Surface 4, PartDesign 2; 3rdParty/salomesmesh 8 |
| `TopExp` | 209 | 117 | Part 120, TechDraw 57, Surface 11, PartDesign 9, Sketcher 4, Import 3, CAM 2, Inspection 2, MeshPart 1; 3rdParty/salomesmesh 197 |
| `gp_Ax2` | 200 | 41 | TechDraw 111, Part 64, Import 10, PartDesign 7, Sketcher 5, CAM 3; 3rdParty/salomesmesh 8, tests 1 |
| `TopLoc_Location` | 190 | 23 | Part 95, Import 49, PartDesign 23, CAM 8, Surface 4, TechDraw 4, Sketcher 3, Measure 2, MeshPart 2; 3rdParty/salomesmesh 54, tests 21 |
| `TopoDS_Vertex` | 188 | 65 | TechDraw 88, Part 69, Sketcher 6, Fem 5, Import 5, Measure 5, PartDesign 5, MeshPart 4, CAM 1; 3rdParty/salomesmesh 264, tests 13 |
| `Geom_BSplineSurface` | 186 | 26 | Part 169, ReverseEngineering 11, Fem 2, MeshPart 2, Surface 2; 3rdParty/salomesmesh 1 |
| `gp_Trsf` | 184 | 27 | Part 68, PartDesign 44, CAM 31, TechDraw 27, Sketcher 7, Import 3, Surface 2, Fem 1, JtReader 1; 3rdParty/salomesmesh 23, tests 23 |
| `TopAbs_WIRE` | 183 | 0 | Part 126, PartDesign 22, CAM 14, TechDraw 11, Sketcher 4, Import 3, Fem 1, Measure 1, Surface 1; 3rdParty/salomesmesh 27, tests 9 |
| `TopAbs_VERTEX` | 179 | 0 | Part 108, TechDraw 27, PartDesign 16, Measure 9, Import 6, Sketcher 6, CAM 3, Surface 3, Fem 1; 3rdParty/salomesmesh 138, tests 10 |
| `Geom_BSplineCurve` | 172 | 22 | Part 138, TechDraw 9, MeshPart 7, Import 6, Sketcher 5, Surface 5, Fem 2; tests 2, 3rdParty/salomesmesh 1 |
| `Geom_Surface` | 160 | 23 | Part 155, TechDraw 2, Fem 1, MeshPart 1, PartDesign 1; 3rdParty/salomesmesh 77 |
| `gp_Pnt2d` | 158 | 16 | Part 147, ReverseEngineering 6, TechDraw 3, Fem 1, MeshPart 1; 3rdParty/salomesmesh 116 |
| `BRepBuilderAPI_MakeEdge` | 156 | 62 | Part 58, TechDraw 51, PartDesign 24, Import 9, CAM 8, Sketcher 4, MeshPart 1, Surface 1; tests 107, 3rdParty/salomesmesh 6 |
| `gp_Pln` | 140 | 57 | Part 63, PartDesign 25, TechDraw 20, Sketcher 12, CAM 9, Fem 6, Measure 3, MeshPart 2; 3rdParty/salomesmesh 7, tests 2 |
| `Geom2d_TrimmedCurve` | 139 | 13 | Part 139; 3rdParty/salomesmesh 5 |
| `TopAbs_ShapeEnum` | 137 | 6 | Part 120, TechDraw 6, PartDesign 5, Fem 2, Measure 2, Sketcher 2; 3rdParty/salomesmesh 91, tests 8 |
| `Geom_Circle` | 123 | 22 | Part 88, Sketcher 12, Measure 10, TechDraw 10, Import 3; 3rdParty/salomesmesh 23 |
| `TopoDS_Compound` | 118 | 45 | Part 45, TechDraw 43, CAM 13, Import 6, PartDesign 6, ReverseEngineering 2, Measure 1, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 7, tests 3 |
| `Geom_Ellipse` | 117 | 12 | Part 96, Sketcher 17, Import 3, TechDraw 1 |
| `Geom_Line` | 116 | 24 | Part 93, TechDraw 16, Measure 3, Fem 2, PartDesign 1, Sketcher 1; 3rdParty/salomesmesh 19 |
| `TDF_Label` | 111 | 8 | Import 111 |
| `gp_Ax1` | 111 | 21 | Part 70, PartDesign 19, TechDraw 15, Import 3, Measure 2, Sketcher 2; 3rdParty/salomesmesh 25, tests 2 |
| `Geom_Plane` | 109 | 33 | Part 89, Sketcher 7, Fem 4, MeshPart 4, PartDesign 2, ReverseEngineering 2, TechDraw 1; 3rdParty/salomesmesh 3 |
| `Geom2d_BSplineCurve` | 108 | 5 | Part 108; 3rdParty/salomesmesh 1 |
| `TopAbs_SOLID` | 107 | 0 | Part 54, PartDesign 24, Import 11, TechDraw 8, CAM 6, Measure 2, Fem 1, Inspection 1; 3rdParty/salomesmesh 119, tests 3 |
| `Geom_Hyperbola` | 99 | 10 | Part 98, Sketcher 1 |
| `BRep_Builder` | 97 | 69 | Part 48, TechDraw 22, CAM 11, Import 6, PartDesign 5, ReverseEngineering 2, Measure 1, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 13 |
| `Geom_BezierCurve` | 93 | 16 | Part 73, Surface 13, TechDraw 5, Fem 2; 3rdParty/salomesmesh 1, tests 1 |
| `Geom_Geometry` | 93 | 2 | Part 90, Sketcher 2, Fem 1 |
| `BRepAdaptor_Surface` | 91 | 59 | Part 40, PartDesign 17, Measure 13, Fem 9, TechDraw 5, Assembly 2, CAM 2, Sketcher 2, Surface 1; 3rdParty/salomesmesh 20, tests 2 |
| `Geom2d_Curve` | 89 | 14 | Part 89; 3rdParty/salomesmesh 34 |
| `Geom2d_Ellipse` | 85 | 5 | Part 85 |
| `Geom_Parabola` | 84 | 11 | Part 81, CAM 2, Sketcher 1 |
| `Geom_Conic` | 83 | 3 | Part 83 |
| `BRepBuilderAPI_MakeFace` | 81 | 47 | Part 50, TechDraw 13, PartDesign 9, Sketcher 5, Surface 3, CAM 1; 3rdParty/salomesmesh 4, tests 4 |
| `GProp_GProps` | 81 | 35 | Part 61, TechDraw 9, Measure 7, PartDesign 3, Fem 1; 3rdParty/salomesmesh 10, tests 3 |
| `Geom_BezierSurface` | 81 | 11 | Part 77, Fem 2, ReverseEngineering 2; 3rdParty/salomesmesh 1, tests 1 |
| `TopAbs_SHAPE` | 80 | 0 | Part 68, PartDesign 5, Sketcher 3, Fem 2, Import 1, TechDraw 1; 3rdParty/salomesmesh 26, tests 6 |
| `TopAbs_SHELL` | 79 | 0 | Part 60, Import 7, PartDesign 5, TechDraw 4, CAM 1, Fem 1, Inspection 1; 3rdParty/salomesmesh 43, tests 2 |
| `Bnd_Box` | 76 | 44 | TechDraw 34, Part 28, CAM 5, PartDesign 5, Fem 4; 3rdParty/salomesmesh 24 |
| `Geom2d_Hyperbola` | 74 | 3 | Part 74 |
| `BRepGProp` | 72 | 35 | Part 55, TechDraw 10, Measure 3, PartDesign 3, Fem 1; 3rdParty/salomesmesh 18, tests 3 |
| `Geom2d_Line` | 72 | 8 | Part 72; 3rdParty/salomesmesh 9 |
| `TopAbs_COMPOUND` | 71 | 0 | Part 60, Import 5, CAM 2, PartDesign 2, Measure 1, TechDraw 1; 3rdParty/salomesmesh 43, tests 8 |
| `gp_Ax3` | 66 | 19 | TechDraw 26, Part 24, CAM 9, Sketcher 5, PartDesign 2; 3rdParty/salomesmesh 16 |
| `GeomAbs_Line` | 65 | 0 | Part 17, Measure 11, TechDraw 11, PartDesign 8, Assembly 6, Sketcher 4, Fem 3, CAM 2, Import 2, Robot 1; 3rdParty/salomesmesh 6, tests 1 |
| `BRepBuilderAPI_MakeWire` | 64 | 29 | Part 40, CAM 10, TechDraw 8, PartDesign 2, Sketcher 2, Import 1, Surface 1; tests 9 |
| `Geom2d_BezierCurve` | 63 | 5 | Part 63; 3rdParty/salomesmesh 1 |
| `Geom2d_Circle` | 62 | 5 | Part 62; 3rdParty/salomesmesh 2 |
| `BRepBndLib` | 61 | 45 | TechDraw 26, Part 21, PartDesign 6, CAM 4, Fem 4; 3rdParty/salomesmesh 13 |
| `BRepTools` | 60 | 36 | TechDraw 42, Part 16, Fem 1, MeshPart 1; 3rdParty/salomesmesh 8 |
| `gp_Lin` | 59 | 22 | Measure 22, Part 21, PartDesign 8, Fem 5, Sketcher 3; 3rdParty/salomesmesh 9 |
| `GeomAbs_Circle` | 57 | 0 | Part 15, Measure 10, TechDraw 9, Assembly 6, PartDesign 6, CAM 5, Sketcher 3, Import 2, Robot 1; 3rdParty/salomesmesh 2 |
| `Geom_CylindricalSurface` | 57 | 9 | Part 57 |
| `TDocStd_Document` | 56 | 17 | Import 56 |
| `Geom2d_Parabola` | 52 | 3 | Part 52 |
| `BRepBuilderAPI_MakeVertex` | 48 | 39 | TechDraw 20, Part 12, Sketcher 4, Fem 3, CAM 2, Measure 2, PartDesign 2, Import 1, Inspection 1, MeshPart 1; tests 5, 3rdParty/salomesmesh 1 |
| `Interface_Static` | 48 | 13 | Part 48 |
| `GeomAbs_Plane` | 46 | 0 | Part 18, PartDesign 12, Assembly 7, Fem 3, Measure 2, Sketcher 2, CAM 1, TechDraw 1; 3rdParty/salomesmesh 6, tests 1 |
| `gp_Dir2d` | 46 | 4 | Part 46; 3rdParty/salomesmesh 2 |
| `GeomAbs_Shape` | 44 | 7 | Part 30, Surface 5, MeshPart 4, ReverseEngineering 3, PartDesign 2; 3rdParty/salomesmesh 2 |
| `Geom_ConicalSurface` | 44 | 6 | Part 44 |
| `Geom2d_Geometry` | 43 | 1 | Part 43 |
| `TopAbs_REVERSED` | 43 | 0 | Part 29, TechDraw 8, CAM 3, Robot 3; 3rdParty/salomesmesh 59, tests 7 |
| `gp_Circ` | 43 | 32 | Part 12, TechDraw 12, Import 6, CAM 4, Sketcher 4, Measure 3, PartDesign 2; tests 1 |
| `gp_XYZ` | 43 | 13 | Part 19, Fem 11, Measure 7, MeshPart 2, Sketcher 2, JtReader 1, PartDesign 1; 3rdParty/salomesmesh 708, tests 15 |
| `GeomAbs_C0` | 42 | 0 | Part 24, Surface 9, TechDraw 4, PartDesign 2, Import 1, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 3 |
| `Geom_ElementarySurface` | 41 | 1 | Part 41 |
| `gp` | 38 | 4 | Part 28, TechDraw 8, Sketcher 2; 3rdParty/salomesmesh 47 |
| `Geom_SphericalSurface` | 37 | 9 | Part 37; 3rdParty/salomesmesh 4 |
| `Geom_SurfaceOfRevolution` | 37 | 5 | Part 37 |
| `Geom_CartesianPoint` | 36 | 6 | Part 36 |
| `Geom_ToroidalSurface` | 36 | 7 | Part 36; 3rdParty/salomesmesh 4 |
| `TopoDS_Shell` | 36 | 17 | Part 35, TechDraw 1; 3rdParty/salomesmesh 27, tests 1 |
| `gp_Vec2d` | 36 | 2 | Part 29, Sketcher 7; 3rdParty/salomesmesh 47 |
| `BRepExtrema_DistShapeShape` | 35 | 26 | Part 10, TechDraw 9, Measure 4, Fem 3, Inspection 3, CAM 2, PartDesign 2, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 3 |
| `BRepLib` | 35 | 16 | TechDraw 22, Part 12, CAM 1 |
| `Geom_RectangularTrimmedSurface` | 35 | 10 | Part 35; 3rdParty/salomesmesh 9 |
| `TopoDS_Iterator` | 34 | 26 | Part 23, TechDraw 4, Import 3, Measure 2, CAM 1, PartDesign 1; 3rdParty/salomesmesh 53 |
| `GeomAbs_C2` | 33 | 0 | Part 27, ReverseEngineering 3, MeshPart 2, Surface 1; 3rdParty/salomesmesh 2 |
| `Geom2d_Conic` | 32 | 2 | Part 32 |
| `Geom_OffsetCurve` | 32 | 5 | Part 32 |
| `math_Matrix` | 32 | 5 | ReverseEngineering 28, Part 2, Surface 2; 3rdParty/salomesmesh 6 |
| `BRepBuilderAPI_Transform` | 30 | 23 | TechDraw 20, Part 10; tests 1 |
| `Geom_SurfaceOfLinearExtrusion` | 30 | 5 | Part 30 |
| `GeomAbs_Cylinder` | 29 | 0 | Assembly 9, Part 8, Measure 7, Fem 3, PartDesign 2; 3rdParty/salomesmesh 4 |
| `TopAbs_COMPSOLID` | 28 | 0 | Part 25, Fem 1, Import 1, TechDraw 1; 3rdParty/salomesmesh 10, tests 1 |
| `BRepBuilderAPI_Copy` | 27 | 32 | TechDraw 12, Part 11, Measure 1, PartDesign 1, Sketcher 1, Surface 1; tests 4, 3rdParty/salomesmesh 2 |
| `Geom_OffsetSurface` | 27 | 4 | Part 27 |
| `gce_ErrorType` | 27 | 2 | Part 27 |
| `BRepCheck_Analyzer` | 26 | 19 | Part 24, CAM 1, TechDraw 1; 3rdParty/salomesmesh 1 |
| `gp_Ax22d` | 26 | 2 | Part 26 |
| `GeomAbs_BSplineCurve` | 24 | 0 | Part 9, TechDraw 8, Fem 2, Sketcher 2, Import 1, Measure 1, Robot 1 |
| `GeomAbs_Ellipse` | 24 | 0 | Part 10, TechDraw 9, Sketcher 3, Import 1, Measure 1; 3rdParty/salomesmesh 2 |
| `GeomAbs_C1` | 23 | 0 | Part 22, MeshPart 1 |
| `Message_ProgressRange` | 23 | 5 | Part 21, Import 2; tests 1 |
| `BRepMesh_IncrementalMesh` | 22 | 19 | TechDraw 13, Part 7, MeshPart 2; 3rdParty/salomesmesh 4 |
| `GCPnts_AbscissaPoint` | 22 | 31 | Part 8, TechDraw 7, Fem 4, Measure 1, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 43 |
| `Quantity_ColorRGBA` | 22 | 7 | Import 22 |
| `Standard_ConstructionError` | 22 | 6 | Part 20, ReverseEngineering 1, Surface 1; 3rdParty/salomesmesh 4 |
| `Geom2d_OffsetCurve` | 21 | 2 | Part 21 |
| `GeomAbs_SurfaceType` | 21 | 2 | Part 19, Assembly 2 |
| `TopoDS_Solid` | 21 | 18 | Part 11, Fem 4, PartDesign 3, TechDraw 3; 3rdParty/salomesmesh 12, tests 3 |
| `gp_Elips` | 21 | 13 | TechDraw 7, Part 6, Sketcher 5, Import 3 |
| `math_Vector` | 21 | 1 | ReverseEngineering 15, Part 3, Surface 3; 3rdParty/salomesmesh 17 |
| `GeomAPI_ProjectPointOnSurf` | 20 | 22 | Part 9, Sketcher 9, Fem 1, ReverseEngineering 1; 3rdParty/salomesmesh 17 |
| `ShapeFix_Wire` | 20 | 15 | Part 13, CAM 2, Surface 2, TechDraw 2, Sketcher 1 |
| `BRepBuilderAPI_MakeShape` | 19 | 7 | Part 19 |
| `GeomAdaptor_Curve` | 19 | 16 | Part 18, TechDraw 1; 3rdParty/salomesmesh 12 |
| `TCollection_ExtendedString` | 19 | 3 | Import 15, JtReader 4 |
| `TDataStd_Name` | 19 | 7 | Import 19 |
| `XCAFDoc_ColorSurf` | 19 | 0 | Import 19 |
| `XCAFDoc_ShapeTool` | 19 | 9 | Import 19 |
| `BRepLProp_CLProps` | 18 | 10 | TechDraw 10, Part 8 |
| `GeomAbs_Sphere` | 18 | 0 | Assembly 12, Part 4, Measure 2; 3rdParty/salomesmesh 4 |
| `XCAFDoc_DocumentTool` | 18 | 7 | Import 18 |
| `BRepAlgoAPI_BooleanOperation` | 17 | 1 | Part 17 |
| `BRepTools_WireExplorer` | 17 | 13 | Part 10, CAM 6, Sketcher 1; 3rdParty/salomesmesh 3 |
| `GeomAbs_G1` | 17 | 0 | Part 15, Surface 2; 3rdParty/salomesmesh 1 |
| `GeomAbs_C3` | 16 | 0 | Part 15, MeshPart 1 |
| `GeomAbs_G2` | 16 | 0 | Part 14, Surface 2 |
| `GeomAbs_Torus` | 16 | 0 | Assembly 10, Part 4, Measure 2; 3rdParty/salomesmesh 2 |
| `GeomLProp_CLProps` | 16 | 9 | Part 13, Sketcher 2, MeshPart 1 |
| `GeomPlate_CurveConstraint` | 16 | 2 | Part 16 |
| `HLRAlgo_Projector` | 16 | 13 | TechDraw 9, Part 4, CAM 1, Import 1, Sketcher 1 |
| `HLRBRep_Algo` | 16 | 8 | TechDraw 8, CAM 2, Import 2, Part 2, Sketcher 2 |
| `Poly_Triangulation` | 16 | 11 | Part 15, MeshPart 1; 3rdParty/salomesmesh 5 |
| `XCAFDoc_ColorGen` | 16 | 0 | Import 16 |
| `GeomAbs_BezierCurve` | 15 | 0 | Part 9, Fem 2, TechDraw 2, Import 1, Measure 1; tests 1 |
| `GeomAbs_CN` | 15 | 0 | Part 15 |
| `GeomAbs_Cone` | 15 | 0 | Assembly 9, Part 4, Measure 1, PartDesign 1; 3rdParty/salomesmesh 3 |
| `GeomAbs_JoinType` | 15 | 0 | Part 14, Sketcher 1 |
| `GeomPlate_PointConstraint` | 15 | 3 | Part 15 |
| `TopAbs_FORWARD` | 15 | 0 | Part 11, TechDraw 4; 3rdParty/salomesmesh 80, tests 8 |
| `BRepLProp_SLProps` | 14 | 12 | Part 9, TechDraw 3, PartDesign 1, Surface 1; 3rdParty/salomesmesh 4 |
| `GeomAPI_ProjectPointOnCurve` | 14 | 17 | Part 6, Sketcher 5, CAM 1, Fem 1, Measure 1; 3rdParty/salomesmesh 6 |
| `GeomPlate_Surface` | 14 | 4 | Part 14 |
| `ShapeAnalysis_FreeBounds` | 14 | 13 | Part 13, CAM 1 |
| `ShapeBuild_ReShape` | 14 | 7 | Part 14; tests 2 |
| `TCollection_HAsciiString` | 14 | 0 | Import 7, Part 7 |
| `TopAbs_IN` | 14 | 0 | Part 7, PartDesign 4, CAM 1, Inspection 1, Sketcher 1; 3rdParty/salomesmesh 12 |
| `GeomAbs_Hyperbola` | 13 | 0 | Part 11, Measure 1, Sketcher 1; 3rdParty/salomesmesh 2 |
| `GeomAbs_Parabola` | 13 | 0 | Part 11, Measure 1, Sketcher 1; 3rdParty/salomesmesh 2 |
| `Geom_BoundedCurve` | 13 | 0 | Part 13 |
| `ShapeExtend_WireData` | 13 | 7 | Surface 6, CAM 3, Part 2, TechDraw 2 |
| `XCAFDoc_ColorTool` | 13 | 7 | Import 13 |
| `BRepBuilderAPI_Sewing` | 12 | 11 | Part 8, PartDesign 3, Surface 1; 3rdParty/salomesmesh 1, tests 1 |
| `BRepPrimAPI_MakePrism` | 12 | 13 | Part 6, TechDraw 6 |
| `GC_MakeArcOfCircle` | 12 | 11 | Part 7, PartDesign 2, TechDraw 2, Sketcher 1 |
| `GeomFill_FillingStyle` | 12 | 2 | Surface 11, Part 1 |
| `Poly_Polygon3D` | 12 | 9 | Part 8, TechDraw 3, MeshPart 1; 3rdParty/salomesmesh 1 |
| `ShapeFix_Face` | 12 | 4 | Part 12 |
| `XCAFDoc_ColorCurv` | 12 | 0 | Import 12 |
| `BRepBuilderAPI_MakePolygon` | 11 | 14 | MeshPart 4, Part 4, ReverseEngineering 2, PartDesign 1; tests 2 |
| `BRepProj_Projection` | 11 | 7 | Part 9, PartDesign 1, TechDraw 1 |
| `GCPnts_UniformAbscissa` | 11 | 13 | Part 6, CAM 3, Import 1, MeshPart 1; 3rdParty/salomesmesh 3 |
| `Geom2d_CartesianPoint` | 11 | 1 | Part 11 |
| `GeomLProp_SLProps` | 11 | 6 | Part 11 |
| `IFSelect_RetDone` | 11 | 0 | Part 7, Import 4 |
| `TopAbs_Orientation` | 11 | 0 | Part 10, TechDraw 1; 3rdParty/salomesmesh 9 |
| `BRepBuilderAPI_MakeSolid` | 10 | 11 | Part 8, PartDesign 2 |
| `GeomAPI_PointsToBSpline` | 10 | 9 | Part 6, TechDraw 2, Import 1, MeshPart 1; tests 2 |
| `ShapeAnalysis` | 10 | 15 | TechDraw 6, Part 3, PartDesign 1; 3rdParty/salomesmesh 11 |
| `ShapeFix_ShapeTolerance` | 10 | 11 | Part 5, PartDesign 3, CAM 1, TechDraw 1; 3rdParty/salomesmesh 2 |
| `Standard_DomainError` | 10 | 2 | Part 10; 3rdParty/salomesmesh 1 |
| `XCAFApp_Application` | 10 | 2 | Import 10 |
| `gp_Ax2d` | 10 | 1 | Part 10; 3rdParty/salomesmesh 9 |
| `BRepOffsetAPI_ThruSections` | 9 | 4 | Part 9; tests 1 |
| `BRepTools_History` | 9 | 3 | Part 9; tests 4 |
| `BRepTools_ReShape` | 9 | 3 | Part 9 |
| `BinTools` | 9 | 2 | Part 9 |
| `GeomAbs_BSplineSurface` | 9 | 0 | Part 7, Fem 2 |
| `GeomAdaptor_Surface` | 9 | 6 | Part 7, CAM 1, TechDraw 1; 3rdParty/salomesmesh 2 |
| `Resource_FormatType` | 9 | 2 | Part 5, Import 4 |
| `ShapeFix_Shape` | 9 | 11 | Part 7, TechDraw 2; 3rdParty/salomesmesh 4 |
| `Standard_Type` | 9 | 2 | JtReader 9; 3rdParty/salomesmesh 3 |
| `TopoDS_Builder` | 9 | 4 | Part 6, CAM 2, PartDesign 1; tests 2 |
| `Adaptor3d_Curve` | 8 | 2 | Part 8; 3rdParty/salomesmesh 23 |
| `BRepFeat_MakePrism` | 8 | 3 | Part 8 |
| `BRepFill_Filling` | 8 | 3 | Surface 8 |
| `GeomAbs_CurveType` | 8 | 4 | Part 6, Assembly 2 |
| `GeomLib_IsPlanarSurface` | 8 | 13 | Part 7, TechDraw 1; 3rdParty/salomesmesh 3 |
| `HLRBRep_HLRToShape` | 8 | 8 | TechDraw 4, CAM 1, Import 1, Part 1, Sketcher 1 |
| `Poly_Triangle` | 8 | 1 | Part 7, MeshPart 1 |
| `Quantity_Color` | 8 | 5 | Import 4, Part 4; 3rdParty/salomesmesh 14 |
| `ShapeConstruct_Curve` | 8 | 7 | Part 7, Surface 1 |
| `TCollection_AsciiString` | 8 | 4 | Import 4, JtReader 2, Part 2; 3rdParty/salomesmesh 37 |
| `Approx_Curve3d` | 7 | 7 | TechDraw 4, Part 2, Import 1 |
| `Approx_ParametrizationType` | 7 | 1 | Part 5, ReverseEngineering 1, Surface 1 |
| `BRepClass3d_SolidClassifier` | 7 | 13 | PartDesign 4, CAM 1, Inspection 1, Part 1; 3rdParty/salomesmesh 6 |
| `BRepClass_FaceClassifier` | 7 | 10 | Part 5, Fem 1, Sketcher 1; 3rdParty/salomesmesh 3 |
| `BRepGProp_Face` | 7 | 8 | Part 3, Fem 2, Inspection 1, PartDesign 1 |
| `BRepOffsetAPI_MakePipeShell` | 7 | 9 | PartDesign 4, Part 3 |
| `BRepPrimAPI_MakeHalfSpace` | 7 | 6 | Part 6, TechDraw 1 |
| `BSplCLib` | 7 | 6 | Part 6, Surface 1 |
| `GC_MakeCircle` | 7 | 7 | Part 5, TechDraw 2; tests 1 |
| `GProp_PrincipalProps` | 7 | 8 | Part 7 |
| `Geom2dAdaptor_Curve` | 7 | 7 | Part 7; 3rdParty/salomesmesh 6 |
| `GeomAbs_BezierSurface` | 7 | 0 | Part 5, Fem 2; tests 1 |
| `GeomConvert` | 7 | 4 | Part 7 |
| `Message_ProgressScope` | 7 | 0 | Part 7 |
| `ShapeExtend_FAIL` | 7 | 0 | Part 7 |
| `TDataStd_TreeNode` | 7 | 2 | Import 7 |
| `XCAFDoc` | 7 | 1 | Import 7 |
| `XCAFDoc_GraphNode` | 7 | 2 | Import 7 |
| `gp_Hypr` | 7 | 5 | Part 6, Sketcher 1 |
| `gp_Mat` | 7 | 1 | Part 6, JtReader 1; 3rdParty/salomesmesh 10 |
| `BRepAdaptor_CompCurve` | 6 | 14 | Part 4, Fem 1, Measure 1; 3rdParty/salomesmesh 6 |
| `BRepBuilderAPI_DisconnectedWire` | 6 | 0 | Part 5, Sketcher 1 |
| `BRepLib_FindSurface` | 6 | 6 | Part 5, CAM 1 |
| `BRepPrimAPI_MakeCylinder` | 6 | 5 | Part 3, PartDesign 2, TechDraw 1 |
| `GCPnts_UniformDeflection` | 6 | 11 | Part 4, CAM 1, MeshPart 1; 3rdParty/salomesmesh 2 |
| `GeomAPI_Interpolate` | 6 | 6 | Part 5, Import 1; 3rdParty/salomesmesh 1 |
| `GeomAbs_Arc` | 6 | 0 | Part 6 |
| `GeomPlate_BuildPlateSurface` | 6 | 4 | Part 6 |
| `Geom_SweptSurface` | 6 | 0 | Part 6 |
| `HLRBRep_PolyAlgo` | 6 | 3 | Part 4, TechDraw 2 |
| `IGESControl_Controller` | 6 | 6 | Part 4, Import 2 |
| `ShapeExtend_Explorer` | 6 | 7 | Part 6 |
| `XCAFDoc_Location` | 6 | 4 | Import 6 |
| `XCAFDoc_ShapeMapTool` | 6 | 2 | Import 6 |
| `gp_Circ2d` | 6 | 4 | Part 6; 3rdParty/salomesmesh 1 |
| `gp_Elips2d` | 6 | 3 | Part 6 |
| `gp_Parab` | 6 | 5 | Part 5, Sketcher 1 |
| `Adaptor2d_Curve2d` | 5 | 3 | Part 5; 3rdParty/salomesmesh 17 |
| `Approx_ChordLength` | 5 | 0 | Part 3, ReverseEngineering 1, Surface 1 |
| `BOPAlgo_CUT` | 5 | 0 | Part 5 |
| `BRepBuilderAPI_GTransform` | 5 | 7 | Part 4, PartDesign 1 |
| `BRepPrimAPI_MakeSphere` | 5 | 5 | Part 3, PartDesign 2 |
| `GCPnts_QuasiUniformDeflection` | 5 | 8 | Part 4, TechDraw 1 |
| `GC_MakeEllipse` | 5 | 5 | Part 4, TechDraw 1 |
| `GC_MakeHyperbola` | 5 | 3 | Part 5 |
| `Geom2dAPI_ProjectPointOnCurve` | 5 | 5 | Part 4, TechDraw 1 |
| `GeomAbs_Intersection` | 5 | 0 | Part 5 |
| `GeomAbs_OffsetCurve` | 5 | 0 | Part 5 |
| `GeomAbs_OtherCurve` | 5 | 0 | Part 5 |
| `GeomFill_StretchStyle` | 5 | 0 | Surface 4, Part 1 |
| `ShapeAnalysis_ShapeTolerance` | 5 | 4 | Part 4, Fem 1 |
| `ShapeFix` | 5 | 2 | Part 5 |
| `TDF_ChildIterator` | 5 | 5 | Import 5 |
| `TNaming_NamedShape` | 5 | 2 | Import 5 |
| `gp_Cylinder` | 5 | 15 | Part 4, Fem 1; 3rdParty/salomesmesh 1 |
| `gp_GTrsf` | 5 | 5 | Part 4, PartDesign 1; 3rdParty/salomesmesh 5 |
| `math_Gauss` | 5 | 6 | ReverseEngineering 3, Part 1, Surface 1; 3rdParty/salomesmesh 2 |
| `Approx_Centripetal` | 4 | 0 | Part 3, ReverseEngineering 1 |
| `Approx_IsoParametric` | 4 | 0 | Part 3, ReverseEngineering 1 |
| `BRepAlgoAPI_BuilderAlgo` | 4 | 0 | Part 4 |
| `BRepBuilderAPI_CurveProjectionFailed` | 4 | 0 | Part 4 |
| `BRepBuilderAPI_FindPlane` | 4 | 4 | Part 4 |
| `BRepBuilderAPI_NoFace` | 4 | 0 | Part 4 |
| `BRepBuilderAPI_NotPlanar` | 4 | 0 | Part 4 |
| `BRepBuilderAPI_ParametersOutOfRange` | 4 | 0 | Part 4 |
| `BRepBuilderAPI_Transformed` | 4 | 0 | Part 2, PartDesign 2 |
| `BRepCheck_Status` | 4 | 2 | Part 4 |
| `BRepExtrema_IsOnEdge` | 4 | 0 | Part 2, CAM 1, PartDesign 1 |
| `BRepIntCurveSurface_Inter` | 4 | 4 | Part 4 |
| `BRepOffsetAPI_MakeOffset` | 4 | 7 | Part 2, Sketcher 2 |
| `BRepOffset_Mode` | 4 | 0 | Part 4 |
| `BRepPrimAPI_MakeRevol` | 4 | 7 | Part 3, PartDesign 1 |
| `Draft` | 4 | 0 | PartDesign 4 |
| `GCPnts_QuasiUniformAbscissa` | 4 | 5 | Part 4 |
| `GCPnts_TangentialDeflection` | 4 | 6 | Part 4; 3rdParty/salomesmesh 1 |
| `GC_MakePlane` | 4 | 2 | Part 4 |
| `GC_MakeSegment` | 4 | 4 | Part 4; 3rdParty/salomesmesh 1 |
| `Geom2dAPI_Interpolate` | 4 | 3 | Part 4 |
| `GeomAPI` | 4 | 4 | Part 4 |
| `GeomAPI_IntCS` | 4 | 7 | MeshPart 2, Fem 1, Part 1 |
| `GeomAPI_IntSS` | 4 | 5 | Part 3, PartDesign 1 |
| `GeomAbs_SurfaceOfExtrusion` | 4 | 0 | Part 4 |
| `GeomAbs_SurfaceOfRevolution` | 4 | 0 | Part 4; 3rdParty/salomesmesh 1 |
| `GeomFill_CoonsStyle` | 4 | 0 | Surface 3, Part 1 |
| `GeomFill_CurvedStyle` | 4 | 0 | Surface 3, Part 1 |
| `OSD_Exception` | 4 | 2 | Import 4 |
| `ShapeFix_Root` | 4 | 1 | Part 4 |
| `ShapeFix_Solid` | 4 | 2 | Part 2, PartDesign 2 |
| `ShapeFix_Wireframe` | 4 | 4 | Part 4; tests 2 |
| `ShapeUpgrade_ShellSewing` | 4 | 5 | Part 4 |
| `Standard_RangeError` | 4 | 1 | ReverseEngineering 4; 3rdParty/salomesmesh 3 |
| `TopAbs_ON` | 4 | 0 | Part 4; 3rdParty/salomesmesh 12 |
| `TopAbs_OUT` | 4 | 0 | Part 3, Fem 1; 3rdParty/salomesmesh 14 |
| `TopAbs_State` | 4 | 2 | Part 3, CAM 1; 3rdParty/salomesmesh 9 |
| `TopoDS_CompSolid` | 4 | 4 | Part 4; tests 1 |
| `XSControl_WorkSession` | 4 | 7 | Part 4 |
| `gp_Hypr2d` | 4 | 3 | Part 4 |
| `gp_Parab2d` | 4 | 2 | Part 4 |
| `Adaptor3d_CurveOnSurface` | 3 | 0 | Part 3 |
| `BOPAlgo_FUSE` | 3 | 0 | Part 3 |
| `BRepAlgoAPI_Section` | 3 | 1 | Part 3 |
| `BRepAlgo_NormalProjection` | 3 | 6 | TechDraw 2, Part 1 |
| `BRepBuilderAPI_MakeEdge2d` | 3 | 3 | Part 3 |
| `BRepBuilderAPI_RightCorner` | 3 | 0 | Part 2, PartDesign 1 |
| `BRepBuilderAPI_RoundCorner` | 3 | 0 | Part 2, PartDesign 1 |
| `BRepBuilderAPI_TransitionMode` | 3 | 0 | Part 3 |
| `BRepCheck_NoError` | 3 | 0 | Part 3 |
| `BRepCheck_NotClosed` | 3 | 0 | Part 3 |
| `BRepExtrema_IsInFace` | 3 | 0 | Part 2, Inspection 1 |
| `BRepExtrema_IsVertex` | 3 | 0 | Part 2, PartDesign 1 |
| `BRepFill_Generator` | 3 | 3 | Part 3 |
| `BRepFilletAPI_MakeChamfer` | 3 | 5 | Part 3 |
| `BRepFilletAPI_MakeFillet` | 3 | 6 | Part 3; tests 1 |
| `BRepLib_MakeWire` | 3 | 3 | Part 3 |
| `BRepOffsetAPI_MakeEvolved` | 3 | 5 | Part 3 |
| `BRepOffsetAPI_MakeFilling` | 3 | 3 | Part 3 |
| `BRepOffsetAPI_MakePipe` | 3 | 4 | Part 2, PartDesign 1 |
| `BRepPrimAPI_MakeBox` | 3 | 12 | Part 2, PartDesign 1; tests 7, 3rdParty/salomesmesh 1 |
| `BRepPrimAPI_MakeCone` | 3 | 4 | Part 2, PartDesign 1 |
| `BRepPrim_Wedge` | 3 | 2 | Part 2, PartDesign 1 |
| `BRepTools_ShapeSet` | 3 | 4 | Part 3 |
| `GC_MakeArcOfEllipse` | 3 | 4 | Part 3 |
| `GC_MakeArcOfHyperbola` | 3 | 4 | Part 3 |
| `GC_MakeArcOfParabola` | 3 | 4 | Part 3 |
| `GC_MakeConicalSurface` | 3 | 2 | Part 3 |
| `GC_MakeCylindricalSurface` | 3 | 2 | Part 3 |
| `GC_MakeLine` | 3 | 3 | Part 3 |
| `Geom2dAPI_InterCurveCurve` | 3 | 4 | Part 3 |
| `Geom2dAPI_PointsToBSpline` | 3 | 2 | Part 3 |
| `GeomAPI_ExtremaCurveCurve` | 3 | 6 | Measure 1, Part 1, TechDraw 1; 3rdParty/salomesmesh 1 |
| `GeomAPI_PointsToBSplineSurface` | 3 | 3 | Part 2, Surface 1 |
| `GeomAbs_OffsetSurface` | 3 | 0 | Part 3; 3rdParty/salomesmesh 1 |
| `GeomAbs_OtherSurface` | 3 | 0 | Part 3 |
| `GeomAbs_Tangent` | 3 | 0 | Part 3 |
| `GeomConvert_BSplineCurveToBezierCurve` | 3 | 4 | TechDraw 2, Part 1 |
| `GeomLib` | 3 | 5 | Part 3; 3rdParty/salomesmesh 4 |
| `Geom_Point` | 3 | 1 | Part 3 |
| `HLRBRep_TypeOfResultingEdge` | 3 | 0 | Part 3 |
| `IntRes2d_IntersectionPoint` | 3 | 1 | Part 3 |
| `Law_Function` | 3 | 1 | Part 2, PartDesign 1 |
| `Message_ProgressIndicator` | 3 | 2 | Part 3 |
| `Poly_PolygonOnTriangulation` | 3 | 4 | Part 3; 3rdParty/salomesmesh 1 |
| `Resource_FormatType_UTF8` | 3 | 0 | Import 2, Part 1 |
| `STEPControl_Reader` | 3 | 4 | Part 2, Import 1 |
| `ShapeAnalysis_Shell` | 3 | 3 | Part 3 |
| `ShapeAnalysis_Surface` | 3 | 6 | Part 2, Fem 1; 3rdParty/salomesmesh 10 |
| `ShapeFix_Edge` | 3 | 0 | Part 3 |
| `ShapeFix_FreeBounds` | 3 | 0 | Part 3 |
| `ShapeFix_Shell` | 3 | 0 | Part 3 |
| `Standard_OutOfRange` | 3 | 5 | PartDesign 3; 3rdParty/salomesmesh 10 |
| `StepData_StepModel` | 3 | 4 | Import 3 |
| `TDF_TagSource` | 3 | 3 | Import 3 |
| `TDF_Tool` | 3 | 2 | Import 3 |
| `TDataStd_Integer` | 3 | 2 | Import 3 |
| `TNaming_UsedShapes` | 3 | 1 | Import 3 |
| `TopoDS_TShape` | 3 | 1 | Part 3; 3rdParty/salomesmesh 8, tests 3 |
| `XCAFDoc_Color` | 3 | 1 | Import 3 |
| `gce_ColinearPoints` | 3 | 0 | Part 3 |
| `gce_ConfusedPoints` | 3 | 0 | Part 3 |
| `gce_MakeParab` | 3 | 3 | Part 3 |
| `math_Householder` | 3 | 2 | ReverseEngineering 3 |
| `APIHeaderSection_MakeHeader` | 2 | 3 | Import 1, Part 1 |
| `BOPAlgo_ArgumentAnalyzer` | 2 | 4 | Part 2; tests 4 |
| `BOPAlgo_COMMON` | 2 | 0 | Part 2 |
| `BOPAlgo_CheckResult` | 2 | 0 | Part 2 |
| `BOPAlgo_Operation` | 2 | 0 | Part 2 |
| `BOPTools_AlgoTools3D` | 2 | 1 | Part 2 |
| `BRepAlgo` | 2 | 3 | PartDesign 2 |
| `BRepAlgoAPI_Cut` | 2 | 1 | TechDraw 2 |
| `BRepAlgo_Image` | 2 | 0 | Part 2 |
| `BRepBuilderAPI_FaceDone` | 2 | 0 | Part 2 |
| `BRepBuilderAPI_FaceError` | 2 | 1 | Part 2 |
| `BRepBuilderAPI_MakeShell` | 2 | 2 | Part 2 |
| `BRepBuilderAPI_NurbsConvert` | 2 | 3 | Part 2 |
| `BRepCheck_IntersectingWires` | 2 | 0 | Part 2 |
| `BRepCheck_InvalidCurveOnSurface` | 2 | 0 | Part 2 |
| `BRepCheck_InvalidPointOnCurve` | 2 | 0 | Part 2 |
| `BRepCheck_InvalidSameParameterFlag` | 2 | 0 | Part 2 |
| `BRepCheck_Result` | 2 | 3 | Part 2 |
| `BRepCheck_UnorientableShape` | 2 | 0 | Part 2 |
| `BRepExtrema_SupportType` | 2 | 1 | Part 1, PartDesign 1 |
| `BRepFill` | 2 | 4 | Part 2 |
| `BRepLib_MakeFace` | 2 | 2 | CAM 1, Part 1 |
| `BRepOffsetAPI_MakeOffsetShape` | 2 | 1 | Part 2 |
| `BRepOffsetAPI_MakeThickSolid` | 2 | 3 | Part 2 |
| `BRepPrimAPI_MakeTorus` | 2 | 4 | Part 1, PartDesign 1 |
| `BRepPrim_Cylinder` | 2 | 2 | Part 1, PartDesign 1 |
| `BinTools_ShapeSet` | 2 | 2 | Part 2 |
| `CPnts_AbscissaPoint` | 2 | 2 | Robot 2 |
| `Geom2dInt_GInter` | 2 | 1 | Part 2; 3rdParty/salomesmesh 1 |
| `GeomAbs_IsoU` | 2 | 0 | Fem 2 |
| `GeomAbs_IsoV` | 2 | 0 | Fem 2 |
| `GeomFill_BSplineCurves` | 2 | 3 | Part 1, Surface 1 |
| `GeomFill_BezierCurves` | 2 | 3 | Part 1, Surface 1 |
| `GeomFill_NSections` | 2 | 3 | Part 1, Surface 1 |
| `GeomFill_Trihedron` | 2 | 0 | Part 1, PartDesign 1 |
| `GeomLProp` | 2 | 2 | Part 2 |
| `GeomPlate_MakeApprox` | 2 | 3 | Part 2 |
| `Geom_BoundedSurface` | 2 | 1 | Surface 2 |
| `HLRBRep_PolyHLRToShape` | 2 | 3 | Part 1, TechDraw 1 |
| `IGESControl_Reader` | 2 | 3 | Part 2 |
| `IGESData_GlobalSection` | 2 | 4 | Import 1, Part 1 |
| `IntRes2d_IntersectionSegment` | 2 | 2 | Part 2 |
| `IntTools_Context` | 2 | 1 | Part 2 |
| `Law_Constant` | 2 | 2 | Part 2 |
| `Message_MsgFile` | 2 | 2 | Part 2 |
| `Plate_Plate` | 2 | 1 | Part 2 |
| `Poly_Connect` | 2 | 2 | Part 2 |
| `Quantity_TOC_sRGB` | 2 | 0 | Import 2 |
| `RWMesh_CoordinateSystem_Zup` | 2 | 0 | Import 2 |
| `STEPControl_AsIs` | 2 | 0 | Import 1, Part 1 |
| `ShapeAnalysis_FreeBoundsProperties` | 2 | 3 | Part 2 |
| `ShapeAnalysis_Wire` | 2 | 1 | Part 2 |
| `ShapeUpgrade_UnifySameDomain` | 2 | 0 | Part 2 |
| `Standard_NoSuchObject` | 2 | 2 | Part 2; 3rdParty/salomesmesh 2 |
| `Standard_Transient` | 2 | 1 | Import 1, Part 1; 3rdParty/salomesmesh 6 |
| `Standard_TypeMismatch` | 2 | 3 | Part 2; 3rdParty/salomesmesh 1, tests 1 |
| `StdFail_NotDone` | 2 | 1 | Part 1, Surface 1 |
| `TDataStd_Real` | 2 | 2 | Import 2 |
| `TopAbs_EXTERNAL` | 2 | 0 | Part 2 |
| `TopAbs_INTERNAL` | 2 | 0 | Part 2; 3rdParty/salomesmesh 34 |
| `gce_BadAngle` | 2 | 0 | Part 2 |
| `gce_BadEquation` | 2 | 0 | Part 2 |
| `gce_Done` | 2 | 0 | Part 2 |
| `gce_IntersectionError` | 2 | 0 | Part 2 |
| `gce_InvertAxis` | 2 | 0 | Part 2 |
| `gce_InvertRadius` | 2 | 0 | Part 2 |
| `gce_MakeDir` | 2 | 2 | Part 2 |
| `gce_MakeLin` | 2 | 2 | Part 2 |
| `gce_NegativeRadius` | 2 | 0 | Part 2 |
| `gce_NullAngle` | 2 | 0 | Part 2 |
| `gce_NullAxis` | 2 | 0 | Part 2 |
| `gce_NullFocusLength` | 2 | 0 | Part 2 |
| `gce_NullRadius` | 2 | 0 | Part 2 |
| `gce_NullVector` | 2 | 0 | Part 2 |
| `gp_Lin2d` | 2 | 5 | Part 2 |
| `Adaptor3d_IsoCurve` | 1 | 2 | Fem 1 |
| `Adaptor3d_Surface` | 1 | 1 | Fem 1; 3rdParty/salomesmesh 9 |
| `BOPAlgo_Builder` | 1 | 2 | TechDraw 1 |
| `BOPAlgo_CheckStatus` | 1 | 0 | Part 1 |
| `BOPAlgo_GlueOff` | 1 | 0 | TechDraw 1 |
| `BRepAlgoAPI_Defeaturing` | 1 | 2 | Part 1 |
| `BRepBuilderAPI_DifferentPointsOnClosedCurve` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_DifferentsPointAndParameter` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_EdgeDone` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_LineThroughIdenticPoints` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_ParameterOutOfRange` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_PointProjectionFailed` | 1 | 0 | Part 1 |
| `BRepBuilderAPI_PointWithInfiniteParameter` | 1 | 0 | Part 1 |
| `BRepCheck_BadOrientation` | 1 | 0 | Part 1 |
| `BRepCheck_BadOrientationOfSubshape` | 1 | 0 | Part 1 |
| `BRepCheck_CheckFail` | 1 | 0 | Part 1 |
| `BRepCheck_EmptyShell` | 1 | 0 | Part 1 |
| `BRepCheck_EmptyWire` | 1 | 0 | Part 1 |
| `BRepCheck_EnclosedRegion` | 1 | 0 | PartDesign 1 |
| `BRepCheck_FreeEdge` | 1 | 0 | Part 1 |
| `BRepCheck_Invalid3DCurve` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidCurveOnClosedSurface` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidDegeneratedFlag` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidImbricationOfWires` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidMultiConnexity` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidPointOnCurveOnSurface` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidPointOnSurface` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidRange` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidSameRangeFlag` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidToleranceValue` | 1 | 0 | Part 1 |
| `BRepCheck_InvalidWire` | 1 | 0 | Part 1 |
| `BRepCheck_Multiple3DCurve` | 1 | 0 | Part 1 |
| `BRepCheck_No3DCurve` | 1 | 0 | Part 1 |
| `BRepCheck_NoCurveOnSurface` | 1 | 0 | Part 1 |
| `BRepCheck_NoSurface` | 1 | 0 | Part 1 |
| `BRepCheck_NotConnected` | 1 | 0 | Part 1 |
| `BRepCheck_RedundantEdge` | 1 | 0 | Part 1 |
| `BRepCheck_RedundantFace` | 1 | 0 | Part 1 |
| `BRepCheck_RedundantWire` | 1 | 0 | Part 1 |
| `BRepCheck_SelfIntersectingWire` | 1 | 0 | Part 1 |
| `BRepCheck_Solid` | 1 | 2 | PartDesign 1 |
| `BRepCheck_SubshapeNotInShape` | 1 | 0 | Part 1 |
| `BRepClass3d` | 1 | 3 | Part 1; 3rdParty/salomesmesh 2 |
| `BRepExtrema_ShapeProximity` | 1 | 2 | Part 1 |
| `BRepFeat_MakeRevol` | 1 | 4 | Part 1 |
| `BRepFeat_SplitShape` | 1 | 3 | Part 1; tests 1 |
| `BRepFill_Contact` | 1 | 0 | Part 1 |
| `BRepFill_ContactOnBorder` | 1 | 0 | Part 1 |
| `BRepFill_NoContact` | 1 | 0 | Part 1 |
| `BRepFill_TypeOfContact` | 1 | 0 | Part 1 |
| `BRepLib_FaceDone` | 1 | 0 | Part 1 |
| `BRepLib_FuseEdges` | 1 | 2 | Part 1 |
| `BRepLib_WireDone` | 1 | 0 | Part 1 |
| `BRepOffsetAPI_DraftAngle` | 1 | 3 | Part 1 |
| `BRepOffsetAPI_NormalProjection` | 1 | 3 | Sketcher 1 |
| `BRepOffset_MakeOffset` | 1 | 2 | Part 1 |
| `BRepOffset_Skin` | 1 | 0 | Part 1 |
| `BRepPrimAPI_MakeRevolution` | 1 | 2 | Part 1 |
| `CSLib` | 1 | 2 | Part 1 |
| `CSLib_D1NuIsNull` | 1 | 0 | Part 1 |
| `CSLib_D1NuIsParallelD1Nv` | 1 | 0 | Part 1 |
| `CSLib_D1NvIsNull` | 1 | 0 | Part 1 |
| `CSLib_NormalStatus` | 1 | 0 | Part 1 |
| `ChFi2d_AnaFilletAlgo` | 1 | 0 | Part 1 |
| `ChFi2d_ChamferAPI` | 1 | 0 | Part 1 |
| `ChFi2d_FilletAPI` | 1 | 0 | Part 1 |
| `ChFi2d_FilletAlgo` | 1 | 0 | Part 1 |
| `Convert_ParameterisationType` | 1 | 0 | Surface 1 |
| `Convert_Polynomial` | 1 | 0 | Surface 1 |
| `ElCLib` | 1 | 5 | Sketcher 1; 3rdParty/salomesmesh 3 |
| `GProp_PGProps` | 1 | 2 | Part 1 |
| `Geom2dAPI_ExtremaCurveCurve` | 1 | 2 | Part 1 |
| `Geom2dConvert` | 1 | 2 | Part 1 |
| `Geom2dConvert_ApproxCurve` | 1 | 2 | Part 1 |
| `Geom2dConvert_BSplineCurveToBezierCurve` | 1 | 2 | Part 1 |
| `Geom2dConvert_CompCurveToBSplineCurve` | 1 | 2 | Part 1 |
| `GeomAdaptor` | 1 | 2 | Part 1 |
| `GeomConvert_ApproxCurve` | 1 | 2 | Part 1 |
| `GeomConvert_ApproxSurface` | 1 | 2 | Part 1 |
| `GeomConvert_BSplineCurveKnotSplitting` | 1 | 4 | Sketcher 1 |
| `GeomConvert_CompCurveToBSplineCurve` | 1 | 2 | Part 1 |
| `GeomFill` | 1 | 3 | Part 1 |
| `GeomFill_CorrectedFrenet` | 1 | 2 | Part 1 |
| `GeomFill_CurveAndTrihedron` | 1 | 2 | Part 1 |
| `GeomFill_EvolvedSection` | 1 | 2 | Part 1 |
| `GeomFill_IsFrenet` | 1 | 0 | PartDesign 1 |
| `GeomFill_Location` | 1 | 0 | Part 1 |
| `GeomFill_LocationLaw` | 1 | 1 | Part 1 |
| `GeomFill_Pipe` | 1 | 2 | Part 1 |
| `GeomFill_SectionLaw` | 1 | 1 | Part 1 |
| `GeomFill_Sweep` | 1 | 2 | Part 1 |
| `GeomLib_Tool` | 1 | 2 | TechDraw 1 |
| `GeomPlate_PlateG0Criterion` | 1 | 2 | Part 1 |
| `HLRAppli_ReflectLines` | 1 | 2 | Part 1 |
| `HLRBRep_IsoLine` | 1 | 0 | Part 1 |
| `HLRBRep_OutLine` | 1 | 0 | Part 1 |
| `HLRBRep_Rg1Line` | 1 | 0 | Part 1 |
| `HLRBRep_RgNLine` | 1 | 0 | Part 1 |
| `HLRBRep_Sharp` | 1 | 0 | Part 1 |
| `IFSelect_GeneralInfo` | 1 | 0 | Part 1 |
| `IFSelect_RetError` | 1 | 0 | Import 1 |
| `IFSelect_RetFail` | 1 | 0 | Import 1 |
| `IFSelect_RetStop` | 1 | 0 | Import 1 |
| `IFSelect_ReturnStatus` | 1 | 0 | Import 1 |
| `IGESCAFControl_Reader` | 1 | 1 | Import 1 |
| `IGESCAFControl_Writer` | 1 | 1 | Import 1 |
| `IGESControl_Writer` | 1 | 2 | Part 1 |
| `IGESData_IGESModel` | 1 | 4 | Import 1 |
| `IGESToBRep_Actor` | 1 | 2 | Import 1 |
| `IMeshTools_Parameters` | 1 | 0 | Part 1 |
| `IntCurveSurface_In` | 1 | 0 | Part 1; 3rdParty/salomesmesh 1 |
| `IntTools_FClass2d` | 1 | 2 | Part 1 |
| `LProp_NotDefined` | 1 | 2 | Part 1 |
| `OSD_Parallel` | 1 | 1 | Part 1 |
| `RWGltf_CafReader` | 1 | 1 | Import 1 |
| `RWGltf_CafWriter` | 1 | 1 | Import 1 |
| `RWGltf_WriterTrsfFormat_Compact` | 1 | 0 | Import 1 |
| `Resource_FormatType_Big5` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1250` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1251` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1252` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1253` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1254` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1255` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1256` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1257` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP1258` | 1 | 0 | Part 1 |
| `Resource_FormatType_CP850` | 1 | 0 | Part 1 |
| `Resource_FormatType_EUC` | 1 | 0 | Part 1 |
| `Resource_FormatType_GB` | 1 | 0 | Part 1 |
| `Resource_FormatType_GBK` | 1 | 0 | Part 1 |
| `Resource_FormatType_NoConversion` | 1 | 0 | Part 1 |
| `Resource_FormatType_SJIS` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_1` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_2` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_3` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_4` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_5` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_6` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_7` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_8` | 1 | 0 | Part 1 |
| `Resource_FormatType_iso8859_9` | 1 | 0 | Part 1 |
| `STEPCAFControl_Reader` | 1 | 1 | Import 1 |
| `STEPCAFControl_Writer` | 1 | 1 | Import 1 |
| `STEPControl_Controller` | 1 | 2 | Part 1 |
| `STEPControl_Writer` | 1 | 2 | Part 1 |
| `ShapeAlgo_AlgoContainer` | 1 | 2 | Part 1 |
| `ShapeAnalysis_Curve` | 1 | 2 | Part 1 |
| `ShapeAnalysis_Edge` | 1 | 3 | Part 1 |
| `ShapeFix_EdgeConnect` | 1 | 0 | Part 1 |
| `ShapeFix_FaceConnect` | 1 | 0 | Part 1 |
| `ShapeFix_FixSmallFace` | 1 | 0 | Part 1 |
| `ShapeFix_FixSmallSolid` | 1 | 0 | Part 1 |
| `ShapeFix_SplitCommonVertex` | 1 | 0 | Part 1 |
| `ShapeFix_SplitTool` | 1 | 0 | Part 1 |
| `ShapeFix_WireVertex` | 1 | 0 | Part 1 |
| `ShapeUpgrade_RemoveInternalWires` | 1 | 2 | Part 1 |
| `Standard` | 1 | 2 | Part 1 |
| `Standard_NullValue` | 1 | 2 | Part 1 |
| `StepElement_AnalysisItemWithinRepresentation` | 1 | 1 | Part 1 |
| `StepVisual_AnnotationCurveOccurrence` | 1 | 1 | Part 1 |
| `StlAPI_Writer` | 1 | 2 | Part 1 |
| `TDF_Attribute` | 1 | 0 | Import 1 |
| `TDF_AttributeIterator` | 1 | 1 | Import 1 |
| `TDataStd` | 1 | 1 | Import 1 |
| `TDocStd_Owner` | 1 | 1 | Import 1 |
| `TopTools_FormatVersion_VERSION_1` | 1 | 0 | Part 1 |
| `Transfer_FinderProcess` | 1 | 2 | Part 1 |
| `XCAFDoc_LayerTool` | 1 | 1 | Import 1 |
| `XCAFDoc_VisMaterial` | 1 | 1 | Import 1 |
| `XCAFDoc_VisMaterialTool` | 1 | 1 | Import 1 |
| `XSControl_TransferWriter` | 1 | 2 | Part 1 |
| `gp_Cone` | 1 | 7 | Part 1; 3rdParty/salomesmesh 2 |
| `gp_Quaternion` | 1 | 6 | Part 1; tests 1 |
| `gp_Sphere` | 1 | 10 | Part 1; 3rdParty/salomesmesh 2 |
| `gp_Torus` | 1 | 5 | Part 1; 3rdParty/salomesmesh 1 |
| `gp_Trsf2d` | 1 | 1 | Part 1; 3rdParty/salomesmesh 2 |
| `gp_XY` | 1 | 7 | Part 1; 3rdParty/salomesmesh 444 |

## Static calls

| Call | Uses | Per module |
|---|---:|---|
| `Precision::Confusion` | 785 | Part 285, Sketcher 224, PartDesign 95, TechDraw 65, CAM 50, Fem 33, Assembly 7, Import 7, Surface 7, Measure 6, ReverseEngineering 3, Base 2, MeshPart 1; 3rdParty/salomesmesh 38 |
| `TopoDS::Edge` | 353 | Part 179, TechDraw 58, Measure 23, PartDesign 19, CAM 17, Sketcher 14, Surface 13, Fem 11, Import 9, MeshPart 7, Assembly 2, Robot 1; 3rdParty/salomesmesh 189, tests 3 |
| `TopoDS::Face` | 225 | Part 126, PartDesign 35, TechDraw 20, Fem 18, Measure 13, Surface 5, Assembly 2, CAM 2, Sketcher 2, Inspection 1, MeshPart 1; 3rdParty/salomesmesh 161, tests 2 |
| `Standard_Failure::Raise` | 196 | Part 161, Surface 24, TechDraw 9, CAM 1, PartDesign 1 |
| `BRep_Tool::Pnt` | 160 | Part 60, TechDraw 60, CAM 10, Measure 7, PartDesign 7, Sketcher 7, Import 4, Fem 2, MeshPart 2, Surface 1; 3rdParty/salomesmesh 62, tests 2 |
| `TopoDS::Wire` | 151 | Part 120, CAM 11, PartDesign 8, TechDraw 5, Import 4, Measure 1, Sketcher 1, Surface 1; 3rdParty/salomesmesh 5 |
| `TopoDS::Vertex` | 106 | Part 54, TechDraw 19, PartDesign 8, Sketcher 8, Measure 7, CAM 4, Import 3, Fem 2, Surface 1; 3rdParty/salomesmesh 109, tests 5 |
| `TopExp::MapShapes` | 95 | Part 70, Surface 9, PartDesign 7, TechDraw 4, Import 2, Inspection 2, Sketcher 1; 3rdParty/salomesmesh 56 |
| `Precision::Angular` | 55 | PartDesign 25, Part 13, Sketcher 8, Measure 5, TechDraw 3, Base 1; 3rdParty/salomesmesh 10 |
| `TopExp::FirstVertex` | 46 | TechDraw 25, Part 19, CAM 1, Sketcher 1; 3rdParty/salomesmesh 39 |
| `BRep_Tool::IsClosed` | 45 | Part 25, CAM 11, TechDraw 8, Sketcher 1; 3rdParty/salomesmesh 3 |
| `TopExp::LastVertex` | 44 | TechDraw 25, Part 17, CAM 1, Sketcher 1; 3rdParty/salomesmesh 30 |
| `BRepTools::Write` | 43 | TechDraw 41, Part 2; 3rdParty/salomesmesh 2 |
| `BRepBndLib::Add` | 37 | Part 17, PartDesign 6, TechDraw 6, CAM 4, Fem 4; 3rdParty/salomesmesh 13 |
| `BRep_Tool::Surface` | 33 | Part 32, MeshPart 1; 3rdParty/salomesmesh 58 |
| `BRep_Tool::Curve` | 30 | Part 15, MeshPart 6, Surface 3, CAM 2, Import 2, TechDraw 2; 3rdParty/salomesmesh 44 |
| `BRepLib::BuildCurves3d` | 29 | TechDraw 20, Part 8, CAM 1 |
| `BRepGProp::SurfaceProperties` | 28 | Part 19, TechDraw 7, Measure 1, PartDesign 1; 3rdParty/salomesmesh 8, tests 1 |
| `Interface_Static::SetCVal` | 26 | Part 26 |
| `Precision::SquareConfusion` | 26 | CAM 13, Part 8, Sketcher 4, Base 1 |
| `TopoDS::Shell` | 26 | Part 24, PartDesign 1, TechDraw 1; 3rdParty/salomesmesh 10 |
| `BRepBndLib::AddOptimal` | 24 | TechDraw 20, Part 4 |
| `BRepGProp::LinearProperties` | 24 | Part 21, TechDraw 2, Fem 1; 3rdParty/salomesmesh 5, tests 1 |
| `Precision::IsInfinite` | 21 | Part 20, Base 1; 3rdParty/salomesmesh 8 |
| `BRepGProp::VolumeProperties` | 20 | Part 15, Measure 2, PartDesign 2, TechDraw 1; 3rdParty/salomesmesh 5, tests 1 |
| `BRep_Tool::Tolerance` | 19 | Part 16, Fem 3; 3rdParty/salomesmesh 33 |
| `Standard_ConstructionError::Raise` | 19 | Part 18, ReverseEngineering 1; 3rdParty/salomesmesh 4 |
| `gp::Resolution` | 19 | Part 19; 3rdParty/salomesmesh 20 |
| `Precision::Approximation` | 18 | Part 15, Sketcher 2, Base 1 |
| `GCPnts_AbscissaPoint::Length` | 17 | Part 5, TechDraw 5, Fem 4, Measure 1, MeshPart 1, Sketcher 1; 3rdParty/salomesmesh 28 |
| `TopoDS::Solid` | 16 | Part 10, TechDraw 3, PartDesign 2, Fem 1; 3rdParty/salomesmesh 3 |
| `TopExp::MapShapesAndAncestors` | 15 | Part 10, PartDesign 2, Surface 2, Sketcher 1; 3rdParty/salomesmesh 14 |
| `Precision::Infinite` | 14 | TechDraw 7, Base 4, PartDesign 2, Part 1; 3rdParty/salomesmesh 25 |
| `BRep_Tool::Triangulation` | 11 | Part 10, MeshPart 1; 3rdParty/salomesmesh 5 |
| `Interface_Static::CVal` | 11 | Part 11 |
| `Precision::Intersection` | 11 | Part 7, Sketcher 3, Base 1 |
| `XCAFDoc_DocumentTool::ShapeTool` | 10 | Import 10 |
| `BRep_Tool::Polygon3D` | 9 | Part 5, TechDraw 3, MeshPart 1; 3rdParty/salomesmesh 1 |
| `TopExp::Vertices` | 9 | Part 4, TechDraw 3, Import 1, MeshPart 1; 3rdParty/salomesmesh 44 |
| `TopoDS::Compound` | 9 | Part 7, TechDraw 2; 3rdParty/salomesmesh 1 |
| `ShapeAnalysis_FreeBounds::ConnectEdgesToWires` | 8 | Part 7, CAM 1 |
| `gp::DZ` | 8 | Part 7, Sketcher 1; 3rdParty/salomesmesh 4 |
| `Interface_Static::SetIVal` | 7 | Part 7 |
| `Precision::PApproximation` | 7 | Sketcher 7 |
| `TDataStd_Name::GetID` | 7 | Import 7 |
| `XCAFDoc_DocumentTool::ColorTool` | 7 | Import 7 |
| `BRepTools::OuterWire` | 6 | Part 5, Fem 1; 3rdParty/salomesmesh 4 |
| `BinTools::PutInteger` | 6 | Part 6 |
| `IGESControl_Controller::Init` | 6 | Part 4, Import 2 |
| `BRepTools::Clean` | 5 | Part 4, MeshPart 1; 3rdParty/salomesmesh 1 |
| `BRepTools::Read` | 5 | Part 4, TechDraw 1 |
| `GeomConvert::SplitBSplineCurve` | 5 | Part 5 |
| `ShapeAnalysis::ContourArea` | 5 | TechDraw 5 |
| `ShapeAnalysis::OuterWire` | 5 | Part 3, PartDesign 1, TechDraw 1; 3rdParty/salomesmesh 1 |
| `XCAFApp_Application::GetApplication` | 5 | Import 5 |
| `BRepLib::OrientClosedSolid` | 4 | Part 4 |
| `BRep_Tool::CurveOnSurface` | 4 | Part 4; 3rdParty/salomesmesh 25 |
| `BSplCLib::Reparametrize` | 4 | Part 4 |
| `GeomAPI::To2d` | 4 | Part 4 |
| `Standard_RangeError::Raise` | 4 | ReverseEngineering 4; 3rdParty/salomesmesh 3 |
| `BRep_Tool::Continuity` | 3 | PartDesign 2, Part 1 |
| `BRep_Tool::PolygonOnTriangulation` | 3 | Part 3; 3rdParty/salomesmesh 1 |
| `BinTools::GetInteger` | 3 | Part 3 |
| `GeomLib::NormEstim` | 3 | Part 3; 3rdParty/salomesmesh 4 |
| `Geom_BSplineSurface::MaxDegree` | 3 | Part 3 |
| `Interface_Static::IVal` | 3 | Part 3 |
| `ShapeAnalysis_FreeBounds::ConnectWiresToWires` | 3 | Part 3 |
| `TDF_Tool::Entry` | 3 | Import 3 |
| `TDataStd_Name::Set` | 3 | Import 3 |
| `XCAFDoc::ColorRefGUID` | 3 | Import 3 |
| `XCAFDoc_Location::GetID` | 3 | Import 3 |
| `XCAFDoc_ShapeMapTool::GetID` | 3 | Import 3 |
| `gp::OX` | 3 | TechDraw 3; 3rdParty/salomesmesh 2 |
| `gp::OY` | 3 | TechDraw 3 |
| `gp::OZ` | 3 | TechDraw 2, Part 1 |
| `BRepAlgo::IsValid` | 2 | PartDesign 2 |
| `BRepAlgoAPI_BooleanOperation::Build` | 2 | Part 2 |
| `BRepLib::BuildCurve3d` | 2 | TechDraw 2 |
| `BRep_Tool::Degenerated` | 2 | Part 2 |
| `BRep_Tool::Parameter` | 2 | Part 2; 3rdParty/salomesmesh 11 |
| `BRep_Tool::Range` | 2 | Import 2; 3rdParty/salomesmesh 16 |
| `BSplCLib::EvalBsplineBasis` | 2 | Part 1, Surface 1 |
| `CPnts_AbscissaPoint::Length` | 2 | Robot 2 |
| `GeomLProp::Continuity` | 2 | Part 2 |
| `Message_MsgFile::LoadFromEnv` | 2 | Part 2 |
| `Precision::PConfusion` | 2 | Part 1, Sketcher 1; 3rdParty/salomesmesh 13 |
| `Precision::Parametric` | 2 | Base 2 |
| `TDataStd_Integer::GetID` | 2 | Import 2 |
| `TNaming_NamedShape::GetID` | 2 | Import 2 |
| `TNaming_UsedShapes::GetID` | 2 | Import 2 |
| `TopoDS::CompSolid` | 2 | Part 2 |
| `XCAFDoc_Color::GetID` | 2 | Import 2 |
| `BOPTools_AlgoTools3D::GetNormalToFaceOnEdge` | 1 | Part 1 |
| `BOPTools_AlgoTools3D::PointNearEdge` | 1 | Part 1 |
| `BRepClass3d::OuterShell` | 1 | Part 1; 3rdParty/salomesmesh 2 |
| `BRepFill::Face` | 1 | Part 1 |
| `BRepFill::Shell` | 1 | Part 1 |
| `BRepTools::Dump` | 1 | Part 1 |
| `BRep_Tool::CurveOnPlane` | 1 | TechDraw 1 |
| `BRep_Tool::NaturalRestriction` | 1 | Part 1 |
| `BSplCLib::MaxDegree` | 1 | Part 1 |
| `CSLib::Normal` | 1 | Part 1 |
| `Draft::Draft` | 1 | PartDesign 1 |
| `Draft::execute` | 1 | PartDesign 1 |
| `Draft::handleChangedPropertyType` | 1 | PartDesign 1 |
| `Draft::mustExecute` | 1 | PartDesign 1 |
| `ElCLib::AdjustPeriodic` | 1 | Sketcher 1 |
| `Geom2dConvert::C0BSplineToC1BSplineCurve` | 1 | Part 1 |
| `GeomAdaptor::MakeCurve` | 1 | Part 1 |
| `GeomConvert::C0BSplineToC1BSplineCurve` | 1 | Part 1 |
| `GeomConvert::CurveToBSplineCurve` | 1 | Part 1 |
| `GeomFill::Surface` | 1 | Part 1 |
| `GeomLib_Tool::Parameter` | 1 | TechDraw 1 |
| `Geom_BSplineCurve::MaxDegree` | 1 | Sketcher 1 |
| `Geom_Circle::get_type_descriptor` | 1 | Import 1 |
| `Geom_Ellipse::get_type_descriptor` | 1 | Import 1 |
| `Interface_Static::SetRVal` | 1 | Part 1 |
| `OSD_Parallel::SetUseOcctThreads` | 1 | Part 1 |
| `Precision::IsNegativeInfinite` | 1 | Base 1 |
| `Precision::IsPositiveInfinite` | 1 | Base 1 |
| `Quantity_Color::StringName` | 1 | Import 1 |
| `STEPControl_Controller::Init` | 1 | Part 1 |
| `ShapeAnalysis_Curve::GetSamplePoints` | 1 | Part 1 |
| `ShapeFix::EncodeRegularity` | 1 | Part 1 |
| `ShapeFix::FixVertexPosition` | 1 | Part 1 |
| `ShapeFix::LeastEdgeSize` | 1 | Part 1 |
| `ShapeFix::RemoveSmallEdges` | 1 | Part 1 |
| `ShapeFix::SameParameter` | 1 | Part 1 |
| `Standard::SetReentrant` | 1 | Part 1 |
| `Standard_NullValue::Raise` | 1 | Part 1 |
| `Standard_TypeMismatch::Raise` | 1 | Part 1; 3rdParty/salomesmesh 1 |
| `TDF_TagSource::GetID` | 1 | Import 1 |
| `TDF_TagSource::NewChild` | 1 | Import 1 |
| `TDataStd::IDList` | 1 | Import 1 |
| `TDataStd_Real::GetID` | 1 | Import 1 |
| `TDataStd_TreeNode::GetDefaultTreeID` | 1 | Import 1 |
| `TDocStd_Owner::GetID` | 1 | Import 1 |
| `TopLoc_Location::ScalePrec` | 1 | Part 1 |
| `TopoDS_Shape::Complement` | 1 | Part 1 |
| `TopoDS_Shape::Compose` | 1 | Part 1 |
| `TopoDS_Shape::EmptyCopy` | 1 | Part 1 |
| `TopoDS_Shape::Location` | 1 | Part 1 |
| `TopoDS_Shape::Orientation` | 1 | Part 1 |
| `TopoDS_Shape::Reverse` | 1 | Part 1 |
| `TopoDS_Shape::TShape` | 1 | Part 1 |
| `XCAFDoc::DatumRefGUID` | 1 | Import 1 |
| `XCAFDoc::DimTolRefGUID` | 1 | Import 1 |
| `XCAFDoc::MaterialRefGUID` | 1 | Import 1 |
| `XCAFDoc::ShapeRefGUID` | 1 | Import 1 |
| `XCAFDoc_ColorTool::GetID` | 1 | Import 1 |
| `XCAFDoc_DocumentTool::VisMaterialTool` | 1 | Import 1 |
| `XCAFDoc_LayerTool::GetID` | 1 | Import 1 |
| `XCAFDoc_Location::Set` | 1 | Import 1 |
| `XCAFDoc_ShapeMapTool::Set` | 1 | Import 1 |
| `XCAFDoc_ShapeTool::GetID` | 1 | Import 1 |
| `XCAFDoc_ShapeTool::GetReferredShape` | 1 | Import 1 |
| `XCAFDoc_ShapeTool::GetSubShapes` | 1 | Import 1 |
| `XCAFDoc_ShapeTool::IsReference` | 1 | Import 1 |
| `gp::Origin` | 1 | Sketcher 1; 3rdParty/salomesmesh 5 |
| `gp::XOY` | 1 | Part 1 |

## Modules

| Module | Distinct names | Uses | Counted in the ranking |
|---|---:|---:|---|
| Part | 543 | 12219 | yes |
| 3rdParty/salomesmesh | 213 | 8155 | no |
| TechDraw | 110 | 3257 | yes |
| PartDesign | 100 | 1193 | yes |
| Import | 114 | 938 | yes |
| tests | 73 | 815 | no |
| Sketcher | 80 | 729 | yes |
| CAM | 66 | 708 | yes |
| Measure | 54 | 460 | yes |
| Fem | 66 | 396 | yes |
| Surface | 59 | 268 | yes |
| MeshPart | 47 | 204 | yes |
| ReverseEngineering | 27 | 133 | yes |
| Assembly | 14 | 79 | yes |
| Inspection | 16 | 30 | yes |
| Robot | 9 | 19 | yes |
| JtReader | 6 | 18 | yes |
| Base | 1 | 15 | yes |
| Mesh | 2 | 2 | yes |

## Names in OCCT's style that the wheel does not export

Set aside rather than counted: FreeCAD's own wrappers such as `*Py` classes, foundation typedefs and macros, container typedefs the bindings do not export under that name, and any name since removed from OCCT.

| Name | Uses | Per module |
|---|---:|---|
| `Standard_Real` | 423 | Part 332, MeshPart 14, PartDesign 12, CAM 11, TechDraw 11, Robot 10, ReverseEngineering 9, Fem 6, Surface 6, Import 4, Inspection 4, Measure 2, Sketcher 2; 3rdParty/salomesmesh 115 |
| `Standard_Integer` | 264 | Part 215, Import 17, TechDraw 13, MeshPart 12, Surface 4, Inspection 1, PartDesign 1, ReverseEngineering 1; 3rdParty/salomesmesh 139 |
| `Standard_Boolean` | 201 | Part 189, Import 6, PartDesign 4, Surface 2; 3rdParty/salomesmesh 58 |
| `Standard_True` | 157 | Part 129, TechDraw 12, PartDesign 4, CAM 3, MeshPart 3, Import 2, Surface 2, Fem 1, Sketcher 1; 3rdParty/salomesmesh 36, tests 6 |
| `Standard_False` | 119 | Part 88, Import 14, PartDesign 6, CAM 4, TechDraw 3, Fem 2, Surface 2; 3rdParty/salomesmesh 31 |
| `TColStd_Array1OfReal` | 118 | Part 78, ReverseEngineering 32, Import 3, MeshPart 2, Surface 2, TechDraw 1; 3rdParty/salomesmesh 9 |
| `gce_ErrorStatusText` | 110 | Part 110 |
| `ShapeFix_WirePy` | 101 | Part 101 |
| `TopTools_IndexedMapOfShape` | 95 | Part 66, Surface 9, TechDraw 7, Import 6, PartDesign 5, Inspection 1, Sketcher 1; 3rdParty/salomesmesh 74 |
| `TopTools_ListOfShape` | 76 | Part 62, PartDesign 6, TechDraw 5, Surface 2, Sketcher 1; 3rdParty/salomesmesh 30 |
| `Standard_CString` | 53 | Part 43, Import 2, TechDraw 2, CAM 1, Fem 1, Measure 1, MeshPart 1, PartDesign 1, ReverseEngineering 1; 3rdParty/salomesmesh 3 |
| `ShapeFix_FacePy` | 48 | Part 48 |
| `TopTools_HSequenceOfShape` | 44 | Part 40, CAM 4 |
| `TColStd_Array1OfInteger` | 37 | Part 26, ReverseEngineering 10, Import 1; 3rdParty/salomesmesh 4 |
| `TopTools_ListIteratorOfListOfShape` | 32 | Part 30, Surface 2; 3rdParty/salomesmesh 42 |
| `HLRBRep_PolyAlgoPy` | 31 | Part 31 |
| `ShapeFix_ShapePy` | 29 | Part 29 |
| `BRepOffsetAPI_MakePipeShellPy` | 28 | Part 28 |
| `BRepOffsetAPI_MakeOffsetFix` | 27 | Part 27 |
| `HLRBRep_AlgoPy` | 25 | Part 25 |
| `ShapeFix_ShellPy` | 24 | Part 24 |
| `BRepAdaptor_HCurve` | 21 | TechDraw 12, Part 6, Import 3 |
| `ShapeFix_SolidPy` | 21 | Part 21 |
| `BRepOffsetAPI_MakeFillingPy` | 18 | Part 18 |
| `ShapeFix_EdgePy` | 18 | Part 18 |
| `ShapeFix_FixSmallFacePy` | 18 | Part 18 |
| `Standard_EXPORT` | 17 | Part 17; 3rdParty/salomesmesh 38 |
| `ShapeFix_WireframePy` | 16 | Part 16 |
| `TDF_LabelSequence` | 16 | Import 16 |
| `TopTools_IndexedDataMapOfShapeListOfShape` | 16 | Part 11, PartDesign 2, Surface 2, Sketcher 1; 3rdParty/salomesmesh 15 |
| `ShapeFix_RootPy` | 14 | Part 14 |
| `BRepBuilderAPI_RefineModel` | 13 | Part 13 |
| `ShapeFix_FixSmallSolidPy` | 12 | Part 12 |
| `Standard_OutOfRange_Raise_if` | 12 | Part 12 |
| `ChFi2d_FilletAPIPy` | 11 | Part 11 |
| `ChFi2d_FilletAlgoPy` | 11 | Part 11 |
| `ShapeFix_WireVertexPy` | 11 | Part 11 |
| `BRepLProp_CurveTool` | 10 | TechDraw 8, Part 2 |
| `ChFi2d_AnaFilletAlgoPy` | 10 | Part 10 |
| `ChFi2d_ChamferAPIPy` | 10 | Part 10 |
| `ShapeFix_EdgeConnectPy` | 10 | Part 10 |
| `ShapeFix_FaceConnectPy` | 10 | Part 10 |
| `ShapeFix_FreeBoundsPy` | 10 | Part 10 |
| `ShapeFix_SplitCommonVertexPy` | 10 | Part 10 |
| `ShapeFix_ShapeTolerancePy` | 9 | Part 9 |
| `ShapeFix_SplitToolPy` | 9 | Part 9 |
| `GeomAdaptor_HCurve` | 8 | Part 8 |
| `TColStd_HArray1OfBoolean` | 8 | Part 8 |
| `TColStd_Array2OfReal` | 7 | Part 6, ReverseEngineering 1 |
| `Adaptor3d_HCurve` | 5 | Part 5 |
| `BRepAdaptor_HCompCurve` | 5 | Part 5 |
| `TColStd_HArray1OfReal` | 5 | Part 5 |
| `TopTools_MapOfShape` | 5 | Part 5; 3rdParty/salomesmesh 49 |
| `Standard_ShortReal` | 4 | Part 4; 3rdParty/salomesmesh 5 |
| `Adaptor3d_HCurveOnSurface` | 3 | Part 3 |
| `BRepCheck_ListIteratorOfListOfStatus` | 3 | Part 3 |
| `BRepExtrema_OverlappedSubShapes` | 3 | Part 3 |
| `Poly_Array1OfTriangle` | 3 | Part 3; 3rdParty/salomesmesh 2 |
| `Standard_RangeError_Raise_if` | 3 | Part 3 |
| `Standard_SStream` | 3 | TechDraw 2, Part 1 |
| `TopoDS_EdgeFromVectors` | 3 | TechDraw 3 |
| `gp_PntCompare` | 3 | Import 3 |
| `gp_PntEqual` | 3 | Import 3 |
| `gp_Pnt_Less` | 3 | PartDesign 3 |
| `Adaptor3d_HSurface` | 2 | Fem 2; 3rdParty/salomesmesh 1 |
| `BOPAlgo_ListIteratorOfListOfCheckResult` | 2 | Part 2 |
| `BOPAlgo_ListOfCheckResult` | 2 | Part 2 |
| `BRepAdaptor_HSurface` | 2 | Fem 2 |
| `BRepBuilderAPI_FaceErrorText` | 2 | Part 2 |
| `BRepTools_Write` | 2 | Part 2 |
| `Geom2dAdaptor_HCurve` | 2 | Part 2 |
| `IntRes2d_SequenceOfIntersectionPoint` | 2 | Part 2 |
| `Quantity_yyerror` | 2 | Base 2 |
| `TColStd_HArray1OfInteger` | 2 | Part 2 |
| `TColStd_ListOfTransient` | 2 | Part 2 |
| `TColStd_SequenceOfReal` | 2 | Part 2; 3rdParty/salomesmesh 4 |
| `TDF_AttributeSequence` | 2 | Import 2 |
| `TDF_IDList` | 2 | Import 2 |
| `TopTools_DataMapOfShapeShape` | 2 | Part 2; 3rdParty/salomesmesh 1 |
| `TopoDS_ShapeHasher` | 2 | Part 2 |
| `Adaptor2d_HCurve2d` | 1 | Part 1 |
| `BRepCheck_ListOfStatus` | 1 | Part 1 |
| `BRepExtrema_MapOfIntegerPackedMapOfInteger` | 1 | Part 1 |
| `BRepTopAdaptor_FClass2d` | 1 | Part 1; 3rdParty/salomesmesh 1 |
| `OSD_OpenStream` | 1 | Part 1 |
| `Quantity_yyparse` | 1 | Base 1 |
| `Standard_OVERRIDE` | 1 | Part 1 |
| `TColStd_HSequenceOfTransient` | 1 | Import 1 |
| `TColStd_IndexedDataMapOfStringString` | 1 | Import 1 |
| `TColStd_ListIteratorOfListOfTransient` | 1 | Part 1 |
| `TDF_LabelMapHasher` | 1 | Import 1 |
| `TDF_ListIteratorOfIDList` | 1 | Import 1 |
| `TopTools_DataMapIteratorOfDataMapOfIntegerListOfShape` | 1 | Part 1 |
| `TopTools_DataMapIteratorOfDataMapOfShapeListOfShape` | 1 | Part 1; 3rdParty/salomesmesh 1 |
| `TopTools_DataMapIteratorOfDataMapOfShapeShape` | 1 | Part 1; 3rdParty/salomesmesh 4 |
| `TopTools_DataMapOfIntegerListOfShape` | 1 | Part 1 |
| `TopTools_DataMapOfIntegerShape` | 1 | Part 1 |
| `TopTools_DataMapOfShapeListOfShape` | 1 | Part 1; 3rdParty/salomesmesh 2 |
| `TopTools_SequenceOfShape` | 1 | Part 1; 3rdParty/salomesmesh 6 |

## `.hxx` includes outside the package list

Not OCCT by this list, so not counted above. Prefixes only.

| Prefix | Includes |
|---|---:|
| `SMESH` | 644 |
| `SMDS` | 366 |
| `StdMeshers` | 244 |
| `SMESHDS` | 119 |
| `TColgp` | 62 |
| `NETGENPlugin` | 61 |
| `MED` | 57 |
| `Utils` | 51 |
| `Objects` | 49 |
| `Extensions` | 36 |
| `GCE2d` | 31 |
| `Basics` | 21 |
| `SMDSAbs` | 17 |
| `Config` | 10 |
| `UNV` | 9 |
| `cxx` | 9 |
| `DriverGMF` | 7 |
| `Exception` | 7 |
| `CxxDebug` | 6 |
| `Version` | 6 |
| `IndirectPythonInterface` | 5 |
| `JtNode` | 5 |
| `OpUtil` | 4 |
| `SALOME` | 4 |
| `UNV2411` | 4 |
| `UNV2412` | 4 |
| `DriverMED` | 3 |
| `GEOMUtils` | 3 |
| `Geom2dLProp` | 3 |
| `UNV164` | 3 |
| `UNV2417` | 3 |
| `UNV2420` | 3 |
| `chrono` | 3 |
| `DriverCGNS` | 2 |
| `ExtensionModule` | 2 |
| `ExtensionOldType` | 2 |
| `ExtensionType` | 2 |
| `ExtensionTypeBase` | 2 |
| `JtAttribute` | 2 |
| `ObjectPool` | 2 |
| `PythonType` | 2 |
| `SALOMEDS` | 2 |
| `JtData` | 1 |
| `JtElement` | 1 |
| `LDOMParser` | 1 |
