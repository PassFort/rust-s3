use std::str::FromStr;

use crate::Result;
use crate::{bucket::CHUNK_SIZE, serde_types::HeadObjectResult};
use async_std::fs::File;
use async_std::path::Path;
use futures::io::{AsyncRead, AsyncReadExt};

/// # Example
/// ```rust,no_run
/// use s3::utils::etag_for_path;
///
/// #[tokio::main]
/// async fn main() {
///     let path = "test_etag";
///     let etag = etag_for_path(path).await.unwrap();
///     println!("{}", etag);
/// }
/// ```
pub async fn etag_for_path(path: impl AsRef<Path>) -> Result<String> {
    let mut file = File::open(path).await?;
    let mut digests = Vec::new();
    let mut chunks = 0;
    loop {
        let chunk = read_chunk(&mut file).await?;
        let digest: [u8; 16] = md5::compute(&chunk).into();
        digests.extend_from_slice(&digest);
        chunks += 1;
        if chunk.len() < CHUNK_SIZE {
            break;
        }
    }
    let digest = format!("{:x}", md5::compute(digests));
    let etag = if chunks <= 1 {
        digest
    } else {
        format!("{}-{}", digest, chunks)
    };
    Ok(etag)
}

pub async fn read_chunk<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    const LOCAL_CHUNK_SIZE: usize = 8388;
    let mut chunk = Vec::with_capacity(CHUNK_SIZE);
    loop {
        let mut buffer = [0; LOCAL_CHUNK_SIZE];
        let mut take = reader.take(LOCAL_CHUNK_SIZE as u64);
        let n = take.read(&mut buffer).await?;
        if n < LOCAL_CHUNK_SIZE {
            buffer.reverse();
            let mut trim_buffer = buffer
                .iter()
                .skip_while(|x| **x == 0)
                .copied()
                .collect::<Vec<u8>>();
            trim_buffer.reverse();
            chunk.extend_from_slice(&trim_buffer);
            chunk.shrink_to_fit();
            break;
        } else {
            chunk.extend_from_slice(&buffer);
            if chunk.len() >= CHUNK_SIZE {
                break;
            } else {
                continue;
            }
        }
    }
    Ok(chunk)
}

pub trait GetAndConvertHeaders {
    fn get_and_convert<T: FromStr>(&self, header: &str) -> Option<T>;
    fn get_string(&self, header: &str) -> Option<String>;
}

impl GetAndConvertHeaders for http::header::HeaderMap {
    fn get_and_convert<T: FromStr>(&self, header: &str) -> Option<T> {
        self.get(header)?.to_str().ok()?.parse::<T>().ok()
    }
    fn get_string(&self, header: &str) -> Option<String> {
        Some(self.get(header)?.to_str().ok()?.to_owned())
    }
}

impl From<&http::HeaderMap> for HeadObjectResult {
    fn from(headers: &http::HeaderMap) -> Self {
        let mut metadata = std::collections::HashMap::new();
        for (key, value) in headers.iter() {
            if key.as_str().starts_with("x-amz-meta-") {
                if let Ok(value) = value.to_str() {
                    metadata.insert(
                        key.as_str()["x-amz-meta-".len()..].to_owned(),
                        value.to_owned(),
                    );
                }
            }
        }
 
        Self {
            accept_ranges: headers.get_string("accept-ranges"),
            content_disposition: headers.get_string("Content-Disposition"),
            content_encoding: headers.get_string("Content-Encoding"),
            content_language: headers.get_string("Content-Language"),
            content_length: headers.get_and_convert("Content-Length"),
            content_type: headers.get_string("Content-Type"),
            delete_marker: headers.get_and_convert("x-amz-delete-marker"),
            e_tag: headers.get_string("ETag"),
            expiration: headers.get_string("x-amz-expiration"),
            expires: headers.get_string("Expires"),
            last_modified: headers.get_string("Last-Modified"),
            metadata: Some(metadata),
            missing_meta: headers.get_and_convert("x-amz-missing-meta"),
            object_lock_legal_hold_status: headers.get_string("x-amz-object-lock-legal-hold"),
            object_lock_mode: headers.get_string("x-amz-object-lock-mode"),
            object_lock_retain_until_date: headers.get_string("x-amz-object-lock-retain-until-date"),
            parts_count: headers.get_and_convert("x-amz-mp-parts-count"),
            replication_status: headers.get_string("x-amz-replication-status"),
            request_charged: headers.get_string("x-amz-request-charged"),
            restore: headers.get_string("x-amz-restore"),
            sse_customer_algorithm: headers.get_string("x-amz-server-side-encryption-customer-algorithm"),
            sse_customer_key_md5: headers.get_string("x-amz-server-side-encryption-customer-key-MD5"),
            ssekms_key_id: headers.get_string("x-amz-server-side-encryption-aws-kms-key-id"),
            server_side_encryption: headers.get_string("x-amz-server-side-encryption"),
            storage_class: headers.get_string("x-amz-storage-class"),
            version_id: headers.get_string("x-amz-version-id"),
            website_redirect_location: headers.get_string("x-amz-website-redirect-location"),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::utils::etag_for_path;
    use std::fs::File;
    use std::io::prelude::*;

    fn object(size: u32) -> Vec<u8> {
        (0..size).map(|_| 33).collect()
    }

    #[tokio::test]
    async fn test_etag() {
        let path = "test_etag";
        std::fs::remove_file(path).unwrap_or_else(|_| {});
        let test: Vec<u8> = object(10_000_000);

        let mut file = File::create(path).unwrap();
        file.write_all(&test).unwrap();

        let etag = etag_for_path(path).await.unwrap();

        std::fs::remove_file(path).unwrap_or_else(|_| {});

        assert_eq!(etag, "ae890066cc055c740b3dc3c8854a643b-2");
    }
}
