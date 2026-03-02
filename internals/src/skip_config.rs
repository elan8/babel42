//! Centralized skip/filter configuration for CMake parsing and dep checks.
//!
//! Two concerns:
//! 1. **CMake spec** — find_package options, scope keywords (from cmake.org docs)
//! 2. **Policy** — packages we skip in dep checks (ament infra, system libs)
//!
//! Generic approach:
//! - CMake options: comprehensive list from spec, not package-specific heuristics
//! - Implicit components: data-driven (package → component list) for Boost/Qt
//! - Dep skip: exact names + prefix patterns (e.g. qt5*, qt6*)
//!
//! Future: load overrides from .babel42.yaml in project root.

// =============================================================================
// CMake spec — find_package() options (NOT package names)
// See: https://cmake.org/cmake/help/latest/command/find_package.html
// =============================================================================

/// All find_package options — skip these when extracting package names.
const FIND_PACKAGE_OPTIONS: &[&str] = &[
    "required",
    "quiet",
    "optional",
    "exact",
    "config",
    "module",
    "no_policy_scope",
    "no_module",
    "names",
    "configs",
    "paths",
    "path_suffixes",
    "hints",
    "no_default_path",
    "no_package_root_path",
    "no_cmake_path",
    "no_cmake_environment_path",
    "no_system_environment_path",
    "no_cmake_package_registry",
    "no_cmake_system_path",
    "no_cmake_system_package_registry",
    "cmake_find_root_path_both",
    "only_cmake_find_root_path",
    "no_cmake_find_root_path",
    "version",
    "components",
    "optional_components",
];

/// Scope keywords in ament_target_dependencies(target SCOPE dep1 dep2)
const CMAKE_SCOPE_KEYWORDS: &[&str] = &["PUBLIC", "PRIVATE", "INTERFACE"];

/// Returns true if s is a find_package option (case-insensitive).
pub fn is_find_package_option(s: &str) -> bool {
    let lower = s.to_lowercase();
    FIND_PACKAGE_OPTIONS.contains(&lower.as_str())
}

/// Returns true if s is a scope keyword (case-insensitive).
pub fn is_scope_keyword(s: &str) -> bool {
    let upper = s.to_uppercase();
    CMAKE_SCOPE_KEYWORDS.contains(&upper.as_str())
}

// =============================================================================
// Packages with implicit component syntax (no COMPONENTS keyword)
// find_package(Boost REQUIRED system filesystem) — system, filesystem are components
// find_package(Qt5 REQUIRED Core Widgets) — Core, Widgets are components
// =============================================================================

/// Packages that accept components as plain args. (package_name, component_names)
const IMPLICIT_COMPONENT_PACKAGES: &[(&str, &[&str])] = &[
    (
        "boost",
        &[
            "system", "filesystem", "date_time", "thread", "serialization",
            "program_options", "regex", "chrono", "atomic", "random",
        ],
    ),
    (
        "qt5",
        &["core", "widgets", "gui", "network", "sql", "test", "concurrent", "xml", "dbus"],
    ),
    (
        "qt6",
        &["core", "widgets", "gui", "network", "sql", "test", "concurrent", "xml", "dbus"],
    ),
];

/// Check if package uses implicit components and returns its component set.
pub fn implicit_components_for(package_lower: &str) -> Option<&'static [&'static str]> {
    let base = if package_lower.starts_with("qt5") {
        "qt5"
    } else if package_lower.starts_with("qt6") {
        "qt6"
    } else {
        package_lower
    };
    IMPLICIT_COMPONENT_PACKAGES
        .iter()
        .find(|(pkg, _)| *pkg == base)
        .map(|(_, comps)| *comps)
}

/// Check if this package name triggers implicit-component mode (we've seen it as first arg).
pub fn is_implicit_component_package(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower == "boost"
        || lower == "qt5"
        || lower.starts_with("qt5")
        || lower == "qt6"
        || lower.starts_with("qt6")
}

// =============================================================================
// Test-only packages — use <test_depend> instead of <depend>
// =============================================================================

/// Packages typically used only in tests; fix hint should suggest <test_depend>.
const TEST_ONLY_PACKAGES: &[&str] = &["ament_cmake_gtest", "ament_cmake_gmock"];

/// Returns true if this package should be declared as test_depend.
pub fn is_test_only_package(name: &str) -> bool {
    let lower = name.to_lowercase();
    TEST_ONLY_PACKAGES.iter().any(|s| *s == lower)
}

// =============================================================================
// Dep check policy — packages we do not report as "missing in package.xml"
// Categories: ament infrastructure, language runtimes, system/vendor libs
// =============================================================================

/// Exact package names to skip in dep/find_package_missing and dep/ament_target_undeclared.
const DEP_SKIP_EXACT: &[&str] = &[
    "ament_cmake",
    "ament_cmake_core",
    "ament_cmake_python",
    "pkgconfig",
    "python3",
    "python",
    "eigen3",
    "eigen",
    "opencv",
    "qt5",
    "qt6",
    "boost",
    "bullet",
    "fcl",
    "octomap",
    "osqp",
    "opengl",
    "glew",
    "freeglut",
    "glut",
    "x11",
    "openmp",
    "core",    // Qt component leaked to ament_target_deps
    "widgets", // Qt component leaked to ament_target_deps
];

/// Prefix patterns — if name starts with one of these, skip.
const DEP_SKIP_PREFIXES: &[&str] = &["qt5", "qt6"];

// =============================================================================
// System/vendor packages — still reported but as Warn, not Error
// =============================================================================

/// Packages typically provided via rosdep/system; severity downgraded to Warn.
const SYSTEM_VENDOR_PACKAGES: &[&str] = &[
    "yaml-cpp",
    "tbb",
    "nanoflann",
    "nlohmann_json",
    "graphicsmagickcpp",
    "bond",
    "bondcpp",
];

/// Returns true if this package is a known system/vendor lib (severity → Warn).
pub fn is_system_vendor_package(name: &str) -> bool {
    let lower = name.to_lowercase();
    SYSTEM_VENDOR_PACKAGES.iter().any(|s| *s == lower)
}

/// Returns true if we should skip reporting this package as missing.
/// project_skip_packages: optional list from .babel42.yaml skip_packages.
pub fn dep_should_skip_package(name: &str, project_skip_packages: Option<&[String]>) -> bool {
    let lower = name.to_lowercase();
    if let Some(skip) = project_skip_packages {
        if skip.iter().any(|s| s.to_lowercase() == lower) {
            return true;
        }
    }
    if DEP_SKIP_EXACT.iter().any(|s| *s == lower) {
        return true;
    }
    DEP_SKIP_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}
