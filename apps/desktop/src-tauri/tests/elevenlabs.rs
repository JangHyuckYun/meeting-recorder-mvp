use desktop_lib::stt::elevenlabs::{
    build_request, is_invalid_keyword_error, segments_from_response, text_fields, ElevenLabsConfig,
    ELEVENLABS_STT_URL,
};
use reqwest::Method;
use uuid::Uuid;

#[test]
fn elevenlabs_parses_words_into_speaker_segments() {
    let recording_id = Uuid::new_v4();
    let body = r#"
    {
        "language_code": "ko",
        "language_probability": 0.98,
        "text": "안녕하세요 반갑습니다",
        "words": [
            {"text": "안녕", "type": "word", "start": 0.1, "end": 0.4, "speaker_id": "speaker_0"},
            {"text": "하세요", "type": "word", "start": 0.4, "end": 0.8, "speaker_id": "speaker_0"},
            {"text": " ", "type": "spacing", "start": 0.8, "end": 0.8},
            {"text": "반갑", "type": "word", "start": 1.2, "end": 1.6, "speaker_id": "speaker_1"},
            {"text": "습니다", "type": "word", "start": 1.6, "end": 2.05, "speaker_id": "speaker_1"}
        ]
    }
    "#;

    let segments = segments_from_response(recording_id, body).unwrap();

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].recording_id, recording_id);
    assert_eq!(segments[0].speaker_label, "화자 1");
    assert_eq!(segments[0].text, "안녕하세요");
    assert_eq!(segments[0].start_ms, 100);
    assert_eq!(segments[0].end_ms, 800);
    assert!(segments[0].is_final);

    assert_eq!(segments[1].speaker_label, "화자 2");
    assert_eq!(segments[1].text, "반갑습니다");
    assert_eq!(segments[1].start_ms, 1200);
    assert_eq!(segments[1].end_ms, 2050);
    assert!(segments[1].is_final);
}

#[test]
fn elevenlabs_missing_speaker_maps_to_unconfirmed() {
    let body = r#"
    {
        "text": "테스트",
        "words": [
            {"text": "테스트", "type": "word", "start": 0.0, "end": 0.5, "speaker_id": null}
        ]
    }
    "#;

    let segments = segments_from_response(Uuid::new_v4(), body).unwrap();

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].speaker_label, "화자 미확인");
    assert_eq!(segments[0].text, "테스트");
}

#[test]
fn elevenlabs_build_request_sets_url_and_auth_header() {
    let client = reqwest::Client::new();
    let cfg = ElevenLabsConfig {
        api_key: "test-api-key".to_string(),
        ..ElevenLabsConfig::default()
    };

    let request = build_request(&client, &cfg, vec![0, 1, 2, 3], "audio.wav")
        .build()
        .unwrap();

    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.url().as_str(), ELEVENLABS_STT_URL);
    assert_eq!(request.headers().get("xi-api-key").unwrap(), "test-api-key");
}

fn field<'a>(fields: &'a [(&'static str, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn elevenlabs_batch_defaults_to_scribe_v2() {
    // scribe_v1 was removed from the API on 2026-07-09; the default must never regress to it.
    let fields = text_fields(&ElevenLabsConfig::default());
    assert_eq!(field(&fields, "model_id"), Some("scribe_v2"));
}

#[test]
fn elevenlabs_keyterms_omitted_when_glossary_is_empty() {
    let fields = text_fields(&ElevenLabsConfig::default());
    assert_eq!(field(&fields, "keyterms"), None);

    // Blank-only entries are still no glossary at all.
    let blank = ElevenLabsConfig {
        keyterms: vec!["".to_string(), "   ".to_string()],
        ..ElevenLabsConfig::default()
    };
    assert_eq!(field(&text_fields(&blank), "keyterms"), None);
}

#[test]
fn elevenlabs_keyterms_sent_as_json_array_when_set() {
    let cfg = ElevenLabsConfig {
        keyterms: vec!["오르카".to_string(), "  ".to_string(), "STT".to_string()],
        ..ElevenLabsConfig::default()
    };
    let fields = text_fields(&cfg);
    let keyterms: Vec<String> =
        serde_json::from_str(field(&fields, "keyterms").expect("keyterms field")).unwrap();
    assert_eq!(keyterms, vec!["오르카".to_string(), "STT".to_string()]);
}

#[test]
fn elevenlabs_keyterms_drop_invalid_jamo_and_keep_valid_terms() {
    let cfg = ElevenLabsConfig {
        keyterms: vec![
            "ㄴㅇㅁㄴㅇㅁㄴㅇ".to_string(),
            " 온톨로지 ".to_string(),
            "English 2".to_string(),
            "온톨로지".to_string(),
        ],
        ..ElevenLabsConfig::default()
    };
    let keyterms: Vec<String> =
        serde_json::from_str(field(&text_fields(&cfg), "keyterms").unwrap()).unwrap();
    assert_eq!(keyterms, vec!["온톨로지", "English 2"]);
}

#[test]
fn elevenlabs_invalid_keyword_error_detection_matches_status_or_param() {
    assert!(is_invalid_keyword_error(
        400,
        r#"{"detail":{"status":"invalid_keyword"}}"#
    ));
    assert!(is_invalid_keyword_error(
        400,
        r#"{"detail":{"param":"keywords"}}"#
    ));
    assert!(!is_invalid_keyword_error(500, "invalid_keyword"));
    assert!(!is_invalid_keyword_error(400, r#"{"detail":{"status":"other"}}"#));
}
