use super::*;

#[test]
fn missing_and_null_fields_take_their_defaults() {
    let body = br#"{"success":true,"data":[
            {"id":1,"url":"https://cdn.example/1.png","score":null,"style":null,"author":{}},
            {"id":2,"url":"https://cdn.example/2.png"}
        ]}"#;
    let images = images(body).unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].score, 0);
    assert_eq!(images[0].style, "");
}

#[test]
fn a_picture_that_cannot_be_read_is_left_out() {
    let body = br#"{"success":true,"data":[
            {"id":1},
            {"id":"two","url":"https://cdn.example/2.png"},
            {"id":3,"url":"https://cdn.example/3.png","width":-1},
            7,
            {"id":4,"url":"https://cdn.example/4.png"}
        ]}"#;
    let ids = images(body)
        .unwrap()
        .iter()
        .map(|image| image.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, [4]);
}

#[test]
fn an_answer_that_failed_gives_its_errors() {
    let body = br#"{"success":false,"errors":["Game not found"]}"#;
    assert_eq!(
        images(body),
        Err(AnswerError::Unsuccessful(vec!["Game not found".to_owned()]))
    );
    assert_eq!(errors(body), ["Game not found"]);
    assert_eq!(errors(br#"{"success":false,"errors":"one"}"#), ["one"]);
    assert_eq!(errors(b"<html>"), Vec::<String>::new());
}

#[test]
fn a_body_that_is_not_an_answer_is_malformed() {
    for body in [
        &b"<html>Sign in</html>"[..],
        b"",
        b"[]",
        b"{\"success\":true,\"data\":{}}",
    ] {
        assert!(
            matches!(images(body), Err(AnswerError::Malformed(_))),
            "{}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn a_success_without_data_has_nothing() {
    assert_eq!(images(br#"{"success":true}"#), Ok(Vec::new()));
    assert_eq!(game(br#"{"success":true,"data":null}"#), Ok(None));
    assert_eq!(games(br#"{"success":true,"data":[]}"#), Ok(Vec::new()));
}
