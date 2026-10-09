use super::Manager;
use serde_json::Value;
impl Manager {
    pub(crate) fn document_service(&self) -> crate::documents::Documents {
        crate::documents::Documents::new()
    }
    pub(crate) async fn documents(&self, input: Value) -> Result<Value, String> {
        self.document_service().execute(input).await
    }
}
