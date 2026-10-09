use crate::*;

/// A child grants access by connecting outwards. Registration never grants management access.
pub(super) async fn automatic(
    state: AppState,
    node: Option<String>,
    secret: Option<String>,
    name: Option<String>,
) -> Result<(StatusCode, Json<CredentialResponse>), (StatusCode, Json<Value>)> {
    let failure = |message: &str| (StatusCode::BAD_REQUEST, Json(json!({"error":message})));
    let node = node
        .and_then(|n| Uuid::parse_str(&n).ok())
        .ok_or_else(|| failure("node_id must be a UUID"))?;
    if node.to_string() == state.node_id {
        return Err(failure("cannot connect a node to itself"));
    }
    let secret = secret
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| failure("registration_secret must contain 64 hex digits"))?;
    let name = name.unwrap_or_else(|| "Crabot".into());
    if name.len() > 255 {
        return Err(failure("name too long"));
    }
    let settings = state
        .store
        .get("settings", "cli_project")
        .await
        .unwrap_or_default();
    let project_id = Uuid::parse_str(settings["project_id"].as_str().unwrap_or(""))
        .map_err(|_| failure("node is not ready"))?;
    let client_id = format!("node-{node}");
    let ak = format!(
        "ak_{}",
        hash_secret(&format!("registration-ak:{node}:{secret}"))
    );
    let sk = format!(
        "sk_{}",
        hash_secret(&format!("registration-sk:{node}:{secret}"))
    );
    state.store.transaction(|data| {
        let rows=data.list("credentials");
        if let Some(row)=rows.iter().find(|r|r["project_id"]==json!(project_id)&&r["client_id"]==client_id) {
            if row["ak"]!=ak || row["sk_hash"]!=hash_secret(&sk) {return Err("node already registered with different credentials".into());}
        } else {
            if rows.iter().filter(|r|r["automatic"]==true).count()>=128 {return Err("peer registration capacity reached".into());}
            data.credential(json!({"ak":ak,"sk_hash":hash_secret(&sk),"project_id":project_id,"client_id":client_id,"role":name,"automatic":true}))?;
        }
        Ok(())
    }).await.map_err(|e|(StatusCode::CONFLICT,Json(json!({"error":e}))))?;
    Ok((
        StatusCode::CREATED,
        Json(CredentialResponse {
            ak,
            sk,
            client_id,
            project_id,
        }),
    ))
}
