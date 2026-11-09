"""Write every name the pinned OCCT wheel exports, one per line, for freecad_occt_usage.py.

    uvx --python 3.13 --from cadquery-ocp==8.0.1.1.0 python tools/census/occt_names.py <out>

Only the exported names are read, as data. A name counts for a package if it is the package's own name or begins with it.
"""

import sys

import OCP


def main(out):
    names = set()
    for package in (name for name in dir(OCP) if not name.startswith("_")):
        module = getattr(OCP, package)
        names.update(name for name in dir(module) if name == package or name.startswith(f"{package}_"))
    with open(out, "w", encoding="utf-8") as file:
        file.write("".join(f"{name}\n" for name in sorted(names)))
    print(f"{len(names)} names")


if __name__ == "__main__":
    main(sys.argv[1])
