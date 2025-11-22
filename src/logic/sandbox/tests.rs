//! Unit tests for sandbox parsing functions.

use crate::logic::sandbox::parse::parse_pkgbuild_deps;

#[test]
/// What: Test parsing dependencies from PKGBUILD with depends= syntax.
///
/// Inputs:
/// - PKGBUILD with standard depends= array.
///
/// Output:
/// - Correctly parsed dependencies.
///
/// Details:
/// - Validates basic dependency parsing works.
fn test_parse_pkgbuild_deps_basic() {
    let pkgbuild = r"
pkgname=test-package
pkgver=1.0.0
depends=('foo' 'bar>=1.2')
makedepends=('make' 'gcc')
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"foo".to_string()));
    assert!(depends.contains(&"bar>=1.2".to_string()));

    assert_eq!(makedepends.len(), 2);
    assert!(makedepends.contains(&"make".to_string()));
    assert!(makedepends.contains(&"gcc".to_string()));

    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing dependencies with depends+= syntax in `package()` function.
///
/// Inputs:
/// - PKGBUILD with depends+= inside `package()` function.
///
/// Output:
/// - Correctly parsed dependencies from depends+=.
///
/// Details:
/// - Validates that depends+= patterns are detected and parsed.
fn test_parse_pkgbuild_deps_append() {
    let pkgbuild = r#"
pkgname=test-package
pkgver=1.0.0
package() {
    depends+=(libcairo.so libdbus-1.so)
    cd $_pkgname
    make DESTDIR="$pkgdir" PREFIX=/usr install
}
"#;

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"libcairo.so".to_string()));
    assert!(depends.contains(&"libdbus-1.so".to_string()));

    assert_eq!(makedepends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing unquoted dependencies.
///
/// Inputs:
/// - PKGBUILD with unquoted dependencies.
///
/// Output:
/// - Correctly parsed unquoted dependencies.
///
/// Details:
/// - Validates that unquoted dependencies are parsed correctly.
fn test_parse_pkgbuild_deps_unquoted() {
    let pkgbuild = r"
pkgname=test-package
depends=(libcairo.so libdbus-1.so)
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"libcairo.so".to_string()));
    assert!(depends.contains(&"libdbus-1.so".to_string()));

    assert_eq!(makedepends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing multi-line dependency arrays.
///
/// Inputs:
/// - PKGBUILD with multi-line depends array.
///
/// Output:
/// - Correctly parsed dependencies from multi-line array.
///
/// Details:
/// - Validates multi-line array parsing works correctly.
fn test_parse_pkgbuild_deps_multiline() {
    let pkgbuild = r"
pkgname=test-package
depends=(
    'foo'
    'bar>=1.2'
    'baz'
)
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(depends.len(), 3);
    assert!(depends.contains(&"foo".to_string()));
    assert!(depends.contains(&"bar>=1.2".to_string()));
    assert!(depends.contains(&"baz".to_string()));

    assert_eq!(makedepends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing makedepends+= syntax.
///
/// Inputs:
/// - PKGBUILD with makedepends+= pattern.
///
/// Output:
/// - Correctly parsed makedepends from += pattern.
///
/// Details:
/// - Validates that makedepends+= is also handled.
fn test_parse_pkgbuild_deps_makedepends_append() {
    let pkgbuild = r"
pkgname=test-package
build() {
    makedepends+=(cmake ninja)
    cmake -B build
}
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(makedepends.len(), 2);
    assert!(makedepends.contains(&"cmake".to_string()));
    assert!(makedepends.contains(&"ninja".to_string()));

    assert_eq!(depends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}
