# Maintainer: Firstpick <your-email@example.com>
pkgname=pacsea
pkgver=0.1.0
pkgrel=1
pkgdesc="Fast TUI for searching, inspecting, and queueing pacman/AUR packages written in Rust"
arch=('any')
url="https://github.com/Firstp1ck/Pacsea"
license=('MIT')
depends=('pacman' 'curl')
optdepends=('paru: for AUR package installation'
            'yay: alternative AUR helper')
makedepends=('cargo' 'git')
source=("$pkgname-$pkgver.tar.gz::https://github.com/Firstp1ck/Pacsea/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')  # Replace with actual checksum when available

build() {
  cd "$pkgname-$pkgver"
  cargo build --release --locked
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

  # Install license
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"

  # Install documentation
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
