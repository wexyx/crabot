use crate::*;

#[derive(Deserialize)]
pub struct Registration {
    #[serde(default)]
    enrollment_token: String,
    node_id: Option<String>,
    registration_secret: Option<String>,
    name: Option<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<Registration>,
) -> Result<(StatusCode, Json<CredentialResponse>), (StatusCode, Json<Value>)> {
    if input.enrollment_token.is_empty() {
        return super::registration::automatic(
            state,
            input.node_id,
            input.registration_secret,
            input.name,
        )
        .await;
    }
    let ak = format!("ak_{}", Uuid::new_v4().simple());
    let sk = format!("sk_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let response=state.store.transaction(|data| {
        let key=hash_secret(&input.enrollment_token);
        let mut token=data.get("enrollment_tokens",&key).cloned().ok_or("invalid_token")?;
        if token["used"]==true || token["expires_at"].as_u64().unwrap_or(0)<=storage::now(){return Err("invalid_token".into());}
        let project_id=Uuid::parse_str(&storage::field(&token,"project_id")).map_err(|_|"invalid_token")?;
        let client_id=storage::field(&token,"client_id");
        data.credential(json!({"ak":ak,"sk_hash":hash_secret(&sk),"project_id":project_id,"client_id":client_id,"role":token["role"]}))?;
        token["used"]=json!(true);
        data.set("enrollment_tokens",&key,token);
        Ok(CredentialResponse{ak,sk,client_id,project_id})
    }).await.map_err(|e| {
        let (status,message)=match e.as_str(){
            "invalid_token"=>(StatusCode::UNAUTHORIZED,"enrollment token is invalid, expired, or already used"),
            "already_exists"=>(StatusCode::CONFLICT,"client_id already registered; use saved credentials or a different ID"),
            _=>(StatusCode::INTERNAL_SERVER_ERROR,"registration persistence failed"),
        };
        (status,Json(json!({"error":message})))
    })?;
    Ok((StatusCode::CREATED, Json(response)))
}
