//! Unit tests for sandbox parsing functions.

use crate::logic::sandbox::parse::{parse_pkgbuild_conflicts, parse_pkgbuild_deps};

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
/// - Correctly parsed dependencies from depends+=, filtering out .so files.
///
/// Details:
/// - Validates that depends+= patterns are detected and parsed.
/// - Validates that .so files (virtual packages) are filtered out.
fn test_parse_pkgbuild_deps_append() {
    let pkgbuild = r#"
pkgname=test-package
pkgver=1.0.0
package() {
    depends+=(foo bar)
    cd $_pkgname
    make DESTDIR="$pkgdir" PREFIX=/usr install
}
"#;

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"foo".to_string()));
    assert!(depends.contains(&"bar".to_string()));

    assert_eq!(makedepends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing unquoted dependencies and filtering .so files.
///
/// Inputs:
/// - PKGBUILD with unquoted dependencies including .so files.
///
/// Output:
/// - Correctly parsed unquoted dependencies, with .so files filtered out.
///
/// Details:
/// - Validates that unquoted dependencies are parsed correctly.
/// - Validates that .so files (virtual packages) are filtered out.
fn test_parse_pkgbuild_deps_unquoted() {
    let pkgbuild = r"
pkgname=test-package
depends=(foo bar libcairo.so libdbus-1.so)
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    // .so files should be filtered out
    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"foo".to_string()));
    assert!(depends.contains(&"bar".to_string()));

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

#[test]
/// What: Test parsing jujutsu-git package scenario with various edge cases.
///
/// Inputs:
/// - PKGBUILD similar to jujutsu-git with multi-line arrays, .so files, and other fields.
///
/// Output:
/// - Correctly parsed dependencies, filtering out .so files and other PKGBUILD fields.
///
/// Details:
/// - Validates that other PKGBUILD fields (arch, pkgdesc, url, license, source) are ignored.
/// - Validates that .so files are filtered out.
/// - Validates that multi-line arrays are parsed correctly.
fn test_parse_pkgbuild_deps_jujutsu_git_scenario() {
    let pkgbuild = r"
pkgname=jujutsu-git
pkgver=0.1.0
pkgdesc=Git-compatible VCS that is both simple and powerful
url=https://github.com/martinvonz/jj
license=(Apache-2.0)
arch=(i686 x86_64 armv6h armv7h)
depends=(
    glibc
    libc.so
    libm.so
)
makedepends=(
    libgit2
    libgit2.so
    libssh2
    libssh2.so)
    openssh
    git)
cargo
checkdepends=()
optdepends=()
source=($pkgname::git+$url)
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    // depends should only contain glibc, .so files filtered out
    assert_eq!(depends.len(), 1);
    assert!(depends.contains(&"glibc".to_string()));

    // makedepends should contain libgit2, libssh2
    // .so files are filtered out
    // Note: openssh, git), and cargo are after the array closes, so they're not part of makedepends
    assert_eq!(makedepends.len(), 2);
    assert!(makedepends.contains(&"libgit2".to_string()));
    assert!(makedepends.contains(&"libssh2".to_string()));

    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test that other PKGBUILD fields are ignored.
///
/// Inputs:
/// - PKGBUILD with various non-dependency fields.
///
/// Output:
/// - Only dependency fields are parsed, other fields are ignored.
///
/// Details:
/// - Validates that fields like arch, pkgdesc, url, license, source are not parsed as dependencies.
fn test_parse_pkgbuild_deps_ignore_other_fields() {
    let pkgbuild = r"
pkgname=test-package
pkgver=1.0.0
pkgdesc=Test package description
url=https://example.com
license=(MIT)
arch=(x86_64)
source=($pkgname-$pkgver.tar.gz)
depends=(foo bar)
makedepends=(make)
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    // Only depends and makedepends should be parsed
    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"foo".to_string()));
    assert!(depends.contains(&"bar".to_string()));

    assert_eq!(makedepends.len(), 1);
    assert!(makedepends.contains(&"make".to_string()));

    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test filtering of invalid package names.
///
/// Inputs:
/// - PKGBUILD with invalid dependency names.
///
/// Output:
/// - Invalid names are filtered out.
///
/// Details:
/// - Validates that names ending with ), containing =, or too short are filtered.
fn test_parse_pkgbuild_deps_filter_invalid_names() {
    // Test filtering of invalid names (using single-line format for reliability)
    let pkgbuild = r"
depends=('valid-package' 'invalid)' '=invalid' 'a' 'valid>=1.0')
";

    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

    // Only valid package names should remain
    // Note: 'invalid)' should be filtered out (ends with ))
    // Note: '=invalid' should be filtered out (starts with =)
    // Note: 'a' should be filtered out (too short)
    // So we should have: valid-package and valid>=1.0
    assert_eq!(depends.len(), 2);
    assert!(depends.contains(&"valid-package".to_string()));
    assert!(depends.contains(&"valid>=1.0".to_string()));

    assert_eq!(makedepends.len(), 0);
    assert_eq!(checkdepends.len(), 0);
    assert_eq!(optdepends.len(), 0);
}

#[test]
/// What: Test parsing conflicts from PKGBUILD with conflicts= syntax.
///
/// Inputs:
/// - PKGBUILD with standard conflicts= array.
///
/// Output:
/// - Correctly parsed conflicts.
///
/// Details:
/// - Validates basic conflict parsing works.
/// - Tests the jujutsu/jujutsu-git scenario.
fn test_parse_pkgbuild_conflicts_basic() {
    let pkgbuild = r"
pkgname=jujutsu-git
pkgver=0.1.0
conflicts=('jujutsu')
";

    let conflicts = parse_pkgbuild_conflicts(pkgbuild);

    assert_eq!(conflicts.len(), 1);
    assert!(conflicts.contains(&"jujutsu".to_string()));
}

#[test]
/// What: Test parsing conflicts with multi-line arrays.
///
/// Inputs:
/// - PKGBUILD with multi-line conflicts array.
///
/// Output:
/// - Correctly parsed conflicts from multi-line array.
///
/// Details:
/// - Validates multi-line array parsing works for conflicts.
fn test_parse_pkgbuild_conflicts_multiline() {
    let pkgbuild = r"
pkgname=pacsea-git
pkgver=0.1.0
conflicts=(
    'pacsea'
    'pacsea-bin'
)
";

    let conflicts = parse_pkgbuild_conflicts(pkgbuild);

    assert_eq!(conflicts.len(), 2);
    assert!(conflicts.contains(&"pacsea".to_string()));
    assert!(conflicts.contains(&"pacsea-bin".to_string()));
}

#[test]
/// What: Test parsing conflicts with version constraints.
///
/// Inputs:
/// - PKGBUILD with conflicts containing version constraints.
///
/// Output:
/// - Correctly parsed conflicts with version constraints stripped.
///
/// Details:
/// - Validates that version constraints are removed from conflict names.
fn test_parse_pkgbuild_conflicts_with_versions() {
    let pkgbuild = r"
pkgname=test-package
conflicts=('old-pkg<2.0' 'new-pkg>=3.0')
";

    let conflicts = parse_pkgbuild_conflicts(pkgbuild);

    assert_eq!(conflicts.len(), 2);
    assert!(conflicts.contains(&"old-pkg".to_string()));
    assert!(conflicts.contains(&"new-pkg".to_string()));
}

#[test]
/// What: Test filtering .so files from conflicts.
///
/// Inputs:
/// - PKGBUILD with conflicts including .so files.
///
/// Output:
/// - .so files are filtered out.
///
/// Details:
/// - Validates that virtual packages (.so files) are filtered from conflicts.
fn test_parse_pkgbuild_conflicts_filter_so() {
    let pkgbuild = r"
pkgname=test-package
conflicts=('foo' 'libcairo.so' 'bar' 'libdbus-1.so=1-64')
";

    let conflicts = parse_pkgbuild_conflicts(pkgbuild);

    // .so files should be filtered out
    assert_eq!(conflicts.len(), 2);
    assert!(conflicts.contains(&"foo".to_string()));
    assert!(conflicts.contains(&"bar".to_string()));
}
