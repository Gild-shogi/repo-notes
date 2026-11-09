use std::{io, path::Path};
use tauri::http::{Request, Response, StatusCode};

use crate::store::DataRoot;

pub fn response(root: &DataRoot, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let loaded = (|| {
        let encoded = request
            .uri()
            .path()
            .strip_prefix('/')
            .ok_or(StatusCode::BAD_REQUEST)?;
        let decoded = percent_encoding::percent_decode_str(encoded)
            .decode_utf8()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let path = Path::new(decoded.as_ref());
        let relative = path
            .strip_prefix(root.path())
            .map_err(|_| StatusCode::FORBIDDEN)?;
        let bytes = root.read(relative).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;
        let mime = tauri::utils::mime_type::MimeType::parse(&bytes, &decoded);
        Ok::<_, StatusCode>((bytes, mime))
    })();
    match loaded {
        Ok((bytes, mime)) => Response::builder()
            .header("Content-Type", mime)
            .header("X-Content-Type-Options", "nosniff")
            .body(bytes)
            .unwrap(),
        Err(status) => Response::builder().status(status).body(Vec::new()).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn request(path: &std::path::Path) -> Request<Vec<u8>> {
        let encoded = percent_encoding::utf8_percent_encode(
            path.to_str().unwrap(),
            percent_encoding::NON_ALPHANUMERIC,
        );
        Request::builder()
            .uri(format!("asset://localhost/{encoded}"))
            .body(Vec::new())
            .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn assets_use_the_resolved_root_and_reject_external_links() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let image = tmp.path().join("画像 #1.svg");
        fs::write(&image, "<svg></svg>").unwrap();
        let root = DataRoot::open(tmp.path()).unwrap();
        let served = response(&root, request(&root.path().join("画像 #1.svg")));
        assert_eq!(served.status(), 200);
        assert_eq!(served.body(), b"<svg></svg>");
        fs::write(outside.path().join("secret.svg"), "secret").unwrap();
        let linked = tmp.path().join("linked.svg");
        std::os::unix::fs::symlink(outside.path().join("secret.svg"), &linked).unwrap();
        assert_eq!(response(&root, request(&root.path().join("linked.svg"))).status(), 403);
        assert_eq!(
            response(&root, request(&outside.path().join("secret.svg"))).status(),
            403
        );
    }
}
