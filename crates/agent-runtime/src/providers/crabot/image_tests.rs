use super::{config::ModelApi, protocol::ProtocolFactory};
use crate::attachments::{AttachmentStore, PreparedAttachments};
use serde_json::json;
#[tokio::test]
async fn each_protocol_encodes_native_image_inputs_without_polluting_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = AttachmentStore::new(dir.path().join("files"));
    let image = store
        .upload("photo.png", b"\x89PNG\r\n\x1a\nfixture")
        .unwrap();
    let (_, prepared) =
        PreparedAttachments::prepare(&store, &image.reference(), dir.path()).unwrap();
    prepared
        .scope(async {
            for (api, kind) in [
                (ModelApi::Chat, "image_url"),
                (ModelApi::Responses, "input_image"),
                (ModelApi::Anthropic, "image"),
            ] {
                let original = vec![json!({"role":"user","content":"analyze"})];
                let history = ProtocolFactory::create(api).with_images(&original);
                assert_eq!(original.len(), 1);
                assert_eq!(history.len(), 2);
                assert_eq!(history[1]["content"][0]["type"], kind);
                assert!(history[1].to_string().contains("image/png"));
            }
        })
        .await;
}
