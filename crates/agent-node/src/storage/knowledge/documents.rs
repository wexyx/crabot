use super::graph::Knowledge;
use lbug::{Connection, Value as DbValue};
use serde_json::{Value, json};

pub(crate) fn schema(db: &Knowledge) -> Result<(), String> {
    for q in [
        "CREATE NODE TABLE Document(id STRING PRIMARY KEY, title STRING, source STRING, content STRING, version INT64, updated_at STRING, created_at INT64)",
        "CREATE NODE TABLE DocChunk(id STRING PRIMARY KEY, ordinal INT64, text STRING)",
        "CREATE REL TABLE DOCUMENT_CHUNK(FROM Document TO DocChunk)",
    ] {
        let _ = db.run(q);
    }
    // Existing instance documents have an unknown original import time. Do not
    // invent one from their last edit time or touch legacy project documents.
    let _ = db.run("ALTER TABLE Document ADD created_at INT64 DEFAULT 0");
    db.rows("MATCH (d:Document) RETURN d.id, d.title, d.source, d.content, d.version, d.updated_at, d.created_at LIMIT 0",vec![])?;
    db.rows(
        "MATCH (d:Document)-[:DOCUMENT_CHUNK]->(c:DocChunk) RETURN c.ordinal, c.text LIMIT 0",
        vec![],
    )?;
    Ok(())
}

fn query(
    conn: &Connection,
    text: &str,
    params: Vec<(&str, DbValue)>,
) -> Result<Vec<Vec<DbValue>>, String> {
    let mut s = conn.prepare(text).map_err(|e| e.to_string())?;
    Ok(conn
        .execute(&mut s, params)
        .map_err(|e| e.to_string())?
        .map(|r| r.into_iter().collect())
        .collect())
}
fn s(v: &str) -> DbValue {
    DbValue::String(v.into())
}
fn number(v: &Value, key: &str, default: usize) -> usize {
    v[key].as_u64().unwrap_or(default as u64).min(1_000_000) as usize
}
fn metadata(row: &[DbValue]) -> Value {
    json!({"id":row[0].to_string(),"title":row[1].to_string(),"source":row[2].to_string(),"version":row[3].to_string().parse::<u64>().unwrap_or(0),"updated_at":row[4].to_string(),"created_at":row[5].to_string().parse::<u64>().unwrap_or(0)})
}

pub(crate) fn execute(db: &Knowledge, input: &Value) -> Result<Value, String> {
    let action = input["action"].as_str().unwrap_or("search");
    let id = input["id"].as_str().unwrap_or("");
    let limit = number(input, "limit", 20).clamp(1, 100);
    let offset = number(input, "offset", 0);
    if action == "search" {
        let text = input["query"].as_str().unwrap_or("").trim();
        if text.len() > 1000 {
            return Err("Search query too long".into());
        }
        let predicate = if text.is_empty() {
            ""
        } else {
            " WHERE (lower(d.title) CONTAINS lower($q) OR lower(d.content) CONTAINS lower($q))"
        };
        let params = || {
            if text.is_empty() {
                vec![]
            } else {
                vec![("q", s(text))]
            }
        };
        let count = db.rows(
            &format!("MATCH (d:Document){predicate} RETURN count(d)"),
            params(),
        )?;
        let total = count[0][0]
            .to_string()
            .parse::<usize>()
            .map_err(|_| "Invalid document count")?;
        let mut args = params();
        args.extend([
            ("offset", DbValue::Int64(offset as i64)),
            ("limit", DbValue::Int64((limit + 1) as i64)),
        ]);
        let rows = db.rows(&format!("MATCH (d:Document){predicate} RETURN d.id,d.title,d.source,d.version,d.updated_at,d.created_at ORDER BY d.created_at DESC,d.id SKIP $offset LIMIT $limit"),args)?;
        let more = rows.len() > limit;
        return Ok(
            json!({"available":true,"total":total,"hits":rows.iter().take(limit).map(|r|metadata(r)).collect::<Vec<_>>(),"truncated":more,"next_offset":if more{Some(offset+limit)}else{None}}),
        );
    }
    if action == "read" {
        let rows = db.rows(
            "MATCH (d:Document) WHERE d.id=$id RETURN d.id,d.title,d.source,d.version,d.updated_at,d.created_at",
            vec![("id", s(id))],
        )?;
        let mut result = metadata(rows.first().ok_or("Document not found")?);
        let chunks=db.rows("MATCH (d:Document)-[:DOCUMENT_CHUNK]->(c:DocChunk) WHERE d.id=$id RETURN c.ordinal,c.text ORDER BY c.ordinal SKIP $offset LIMIT $limit",vec![("id",s(id)),("offset",DbValue::Int64(offset as i64)),("limit",DbValue::Int64((limit+1) as i64))])?;
        result["chunks"]=json!(chunks.iter().take(limit).map(|r|json!({"index":r[0].to_string().parse::<u64>().unwrap_or(0),"text":r[1].to_string()})).collect::<Vec<_>>());
        result["truncated"] = json!(chunks.len() > limit);
        result["next_offset"] = json!((chunks.len() > limit).then_some(offset + limit));
        result["untrusted_content"] = json!(true);
        return Ok(result);
    }
    if !matches!(action, "save" | "delete") {
        return Err("Unknown document action".into());
    }
    let title = input["title"].as_str().unwrap_or("").trim();
    let content = input["content"].as_str().unwrap_or("");
    let source = input["source"].as_str().unwrap_or("");
    if action == "save"
        && (title.is_empty()
            || title.len() > 1024
            || content.trim().is_empty()
            || content.len() > 4 * 1024 * 1024
            || source.len() > 4096)
    {
        return Err("Document needs title and text (maximum 4 MiB)".into());
    }
    db.transaction(|conn| {
        let existing=if id.is_empty(){vec![]}else{query(conn,"MATCH (d:Document) WHERE d.id=$id RETURN d.id,d.title,d.source,d.version,d.updated_at,d.created_at",vec![("id",s(id))])?};
        if !id.is_empty() && existing.is_empty(){ return Err("Document not found".into()); }
        let version=existing.first().map(|r|metadata(r)["version"].as_u64().unwrap_or(0)).unwrap_or(0);
        if !id.is_empty() && input["expected_version"].as_u64()!=Some(version){return Err("Document version conflict; reload before editing".into());}
        if action=="delete" && id.is_empty(){return Err("Document id required".into());}
        if !source.is_empty() && action=="save" {
            let same=query(conn,"MATCH (d:Document) WHERE d.source=$source RETURN d.id,d.title,d.source,d.version,d.updated_at,d.created_at",vec![("source",s(source))])?;
            if let Some(other)=same.first().filter(|r|r[0].to_string()!=id) {
                if id.is_empty(){let mut found=metadata(other);found["already_exists"]=json!(true);return Ok(found);}
                return Err("Another document already has this source".into());
            }
        }
        let id=if id.is_empty(){uuid::Uuid::new_v4().to_string()}else{id.into()};
        if version>0 {
            query(conn,"MATCH (d:Document)-[:DOCUMENT_CHUNK]->(c:DocChunk) WHERE d.id=$id DETACH DELETE c",vec![("id",s(&id))])?;
        }
        if action=="delete"{
            query(conn,"MATCH (d:Document {id:$id}) DETACH DELETE d",vec![("id",s(&id))])?;
            return Ok(json!({"id":id,"deleted":true}));
        }
        let updated=crate::storage::now().to_string();
        let created=existing.first().map(|row|metadata(row)["created_at"].as_u64().unwrap_or(0)).unwrap_or_else(||std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64);
        query(conn,"MERGE (d:Document {id:$id}) SET d.title=$title,d.source=$source,d.content=$content,d.version=$version,d.updated_at=$updated,d.created_at=$created",vec![("id",s(&id)),("title",s(title)),("source",s(source)),("content",s(content)),("version",DbValue::Int64((version+1) as i64)),("updated",s(&updated)),("created",DbValue::Int64(created as i64))])?;
        // Chunk boundaries are Unicode safe and deterministic, so references can cite id/version/index.
        insert_chunks(conn,&id,content)?;
        Ok(json!({"id":id,"title":title,"source":source,"version":version+1,"updated_at":updated,"created_at":created}))
    })
}

fn insert_chunks(conn: &Connection, id: &str, content: &str) -> Result<(), String> {
    let chars = content.chars().collect::<Vec<_>>();
    for (n, chunk) in chars.chunks(1600).enumerate() {
        query(
            conn,
            "CREATE (c:DocChunk {id:$chunk,ordinal:$ordinal,text:$text})",
            vec![
                ("chunk", s(&format!("{id}/{n}"))),
                ("ordinal", DbValue::Int64(n as i64)),
                ("text", s(&chunk.iter().collect::<String>())),
            ],
        )?;
        query(
            conn,
            "MATCH (d:Document {id:$id}),(c:DocChunk {id:$chunk}) CREATE (d)-[:DOCUMENT_CHUNK]->(c)",
            vec![("id", s(id)), ("chunk", s(&format!("{id}/{n}")))],
        )?;
    }
    Ok(())
}
