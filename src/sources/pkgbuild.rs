use crate::state::{PackageItem, Source};
use crate::util::percent_encode;

type Result<T> = super::Result<T>;

pub async fn fetch_pkgbuild_fast(item: &PackageItem) -> Result<String> {
    match &item.source {
        Source::Aur => {
            let url = format!(
                "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
                percent_encode(&item.name)
            );
            let res = tokio::task::spawn_blocking(move || super::curl_text(&url)).await??;
            Ok(res)
        }
        Source::Official { .. } => {
            let name = item.name.clone();
            let url_main = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
                percent_encode(&name)
            );
            if let Ok(Ok(txt)) = tokio::task::spawn_blocking({
                let u = url_main.clone();
                move || super::curl_text(&u)
            })
            .await
            {
                return Ok(txt);
            }
            let url_master = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
                percent_encode(&name)
            );
            let txt = tokio::task::spawn_blocking(move || super::curl_text(&url_master)).await??;
            Ok(txt)
        }
    }
}
