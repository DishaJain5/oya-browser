/** Shared message codecs retain wire compatibility without any service process. */
use mars_core::{
    constants::FRAME_BYTES,
    http::Method,
    protocol::{EngineError, Request, Response, decode, encode, read_frame},
};
use std::io::Cursor;

/** Application extraction preserves the established method representation and safe default. */
#[test]
fn preserves_http_method_serialization_and_default() {
    assert_eq!(Method::default(), Method::Get);
    assert_eq!(serde_json::to_string(&Method::Get).unwrap(), "\"Get\"");
    assert_eq!(serde_json::to_string(&Method::Post).unwrap(), "\"Post\"");
    assert_eq!(
        serde_json::from_str::<Method>("\"Post\"").unwrap(),
        Method::Post
    );
    assert!(serde_json::from_str::<Method>("\"Delete\"").is_err());
}

/** Sequential frame transport preserves request identity and consumes complete lines only. */
#[test]
fn round_trips_a_validated_request_through_the_public_codec() {
    let request = Request::new("consumer", "runtime.capabilities", serde_json::json!({}));
    request.validate().unwrap();
    let mut reader = Cursor::new(encode(&request).unwrap());
    let decoded: Request = decode(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.id, request.id);
    assert_eq!(decoded.op, request.op);
    assert!(read_frame(&mut reader).unwrap().is_none());
}

/** Disconnect cannot promote an unterminated candidate into a valid request. */
#[test]
fn rejects_incomplete_and_oversized_frames() {
    assert_eq!(
        read_frame(&mut Cursor::new(b"{}")).unwrap_err().code,
        "invalid_request"
    );
    let oversized = vec![b'x'; FRAME_BYTES + 1];
    assert_eq!(
        read_frame(&mut Cursor::new(oversized)).unwrap_err().code,
        "limit_exceeded"
    );
}

/** Typed errors retain their stable classification and explicit no-retry semantics. */
#[test]
fn preserves_error_identity_across_the_response_codec() {
    let response = Response::new(
        Some("consumer".into()),
        Err(EngineError::unsupported("unsupported input")),
    );
    let decoded: Response = decode(&encode(&response).unwrap()).unwrap();
    assert!(decoded.result.is_none());
    let error = decoded.error.unwrap();
    assert_eq!(error.code, "unsupported_feature");
    assert!(!error.retryable);
    assert_eq!(
        error.hint,
        "Use the supported subset documented in mars/README.md."
    );
}
