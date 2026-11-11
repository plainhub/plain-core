use super::*;
use bytes::Bytes;
#[tokio::test]
async fn collecting_one_chunk_keeps_its_storage_and_enforces_limits() {
    let bytes = Bytes::from(vec![9; 4096]);
    let pointer = bytes.as_ptr();
    let result = Full::new(bytes.clone()).collect().await.unwrap().to_bytes();
    assert_eq!(result.as_ptr(), pointer);
    assert!(
        Limited::new(Full::new(bytes.clone()), 4095)
            .collect()
            .await
            .is_err()
    );
    assert_eq!(
        Limited::new(Full::new(bytes), 4096)
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .len(),
        4096
    );
}
