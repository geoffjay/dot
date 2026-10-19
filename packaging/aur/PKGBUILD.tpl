# Maintainer: Geoff Johnson <geoff.jay@gmail.com>
# PKGBUILD for gdot-bin — binary package sourcing the release tarball from
# GitHub. Regenerated per release by scripts/gen-aur-pkgbuild.sh (URLs and
# checksums are substituted verbatim; the checked-in template is never
# modified by hand per-release).
pkgname=gdot-bin
pkgver=@@VERSION@@
pkgrel=1
pkgdesc="Modular, versioned dotfile manager"
arch=("aarch64" "x86_64")
url="https://github.com/geoffjay/dot"
license=("MIT" "Apache-2.0")
provides=("dot")
conflicts=("dot" "dot-git")

source_aarch64=("@@URL_AARCH64@@")
sha256sums_aarch64=('@@SHA256_AARCH64@@')

source_x86_64=("@@URL_X86_64@@")
sha256sums_x86_64=('@@SHA256_X86_64@@')

package() {
    install -Dm755 "dot" "${pkgdir}/usr/bin/dot"
    install -Dm644 LICENSE-MIT "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-MIT"
    install -Dm644 LICENSE-APACHE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-APACHE"
}
