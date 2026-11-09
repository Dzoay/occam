"""The recorded machine: whitelisted facts only, never a hostname, path or user name."""

import importlib.metadata
import os
import pathlib
import platform
import shutil
import subprocess

_CPU_ROOT = pathlib.Path("/sys/devices/system/cpu")
_PROC = pathlib.Path("/proc")


def _cpu_directories():
    """Every online logical CPU's sysfs directory."""
    if not _CPU_ROOT.is_dir():
        return []
    return sorted(
        (path for path in _CPU_ROOT.glob("cpu[0-9]*") if (path / "topology").is_dir()),
        key=lambda path: int(path.name[3:]),
    )


def physical_cpus():
    """One logical CPU per physical core, lowest number first: the affinity set for 'physical cores only'."""
    seen, chosen = set(), []
    for directory in _cpu_directories():
        package = (directory / "topology" / "physical_package_id").read_text(encoding="ascii").strip()
        core = (directory / "topology" / "core_id").read_text(encoding="ascii").strip()
        if (package, core) not in seen:
            seen.add((package, core))
            chosen.append(int(directory.name[3:]))
    return chosen


def _cpu_model():
    try:
        for line in (_PROC / "cpuinfo").read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def _governor():
    try:
        return (_CPU_ROOT / "cpu0" / "cpufreq" / "scaling_governor").read_text(encoding="ascii").strip()
    except OSError:
        return "unknown"


def _memory_gib():
    try:
        for line in (_PROC / "meminfo").read_text(encoding="ascii").splitlines():
            if line.startswith("MemTotal:"):
                return round(int(line.split()[1]) / (1024 * 1024), 1)
    except (OSError, ValueError, IndexError):
        pass
    return None


def _node_version():
    node = shutil.which("node")
    if node is None:
        return "not installed"
    try:
        return subprocess.run([node, "--version"], capture_output=True, text=True, check=True, timeout=30).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return "unknown"


def wheel_version():
    return f"cadquery-ocp {importlib.metadata.version('cadquery-ocp')}"


def record():
    """The machine as the report names it. Whitelisted fields only."""
    return {
        "cpu": _cpu_model(),
        "physical_cores": len(physical_cpus()),
        "logical_cores": os.cpu_count(),
        "governor": _governor(),
        "kernel": platform.release(),
        "memory_gib": _memory_gib(),
        "wheel": wheel_version(),
        "python": platform.python_version(),
        "node": _node_version(),
    }


def load_average():
    return [round(value, 2) for value in os.getloadavg()]
