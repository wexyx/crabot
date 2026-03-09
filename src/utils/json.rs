use anyhow::{Error, Ok};
use log::error;
use serde::de;
use serde_json::Value;

use crate::biz_err;

pub fn convert<'a, T>(data: &'a str) -> Result<T, Error>
where
    T: de::Deserialize<'a>,
{
    let data: serde_json::Result<T> = serde_json::from_str(data);
    let result = data?;
    return Ok(result);
}

pub fn to_json<T>(data: &T) -> Result<String, Error>
where
    T: serde::Serialize,
{
    let result = serde_json::to_string(data)?;
    return Ok(result);
}

pub fn to_string<T>(data: &T) -> Option<String>
where
    T: serde::Serialize,
{
    let result = serde_json::to_string(data).map(Some).unwrap_or_else(|e| {
        error!("json parse err: {}", e);
        return None;
    });
    return result;
}

#[allow(dead_code)]
pub fn to_string_pretty<T>(data: &T) -> Option<String>
where
    T: serde::Serialize,
{
    let result = serde_json::to_string_pretty(data)
        .map(Some)
        .unwrap_or_else(|e| {
            error!("json parse err: {}", e);
            return None;
        });
    return result;
}

#[allow(dead_code)]
pub fn merge_json_string(left: String, right: String) -> Result<String, Error> {
    if left.len() == 0 {
        return Ok(right);
    }
    if right.len() == 0 {
        return Ok(left);
    }

    let v1: Value = serde_json::from_str(left.as_str())?;
    let v2: Value = serde_json::from_str(right.as_str())?;
    return merge_json_value(v1, v2);
}

pub fn merge_json_value(left: Value, right: Value) -> Result<String, Error> {
    // Ensure both are JSON objects
    if let (Value::Object(mut map1), Value::Object(map2)) = (left, right) {
        // Extend map1 with map2
        map1.extend(map2.into_iter());
        // Convert the merged Value back to a JSON string
        return serde_json::to_string(&map1).map_err(|e| biz_err!(e.to_string()));
    }

    return Ok("{}".to_string());
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Default, Debug, Deserialize, Serialize)]
    struct Demo {
        name: String,
        age: i32,
    }

    #[derive(Default, Debug, Deserialize, Serialize)]
    struct Demo2 {
        nick: String,
        level: i32,
    }

    #[test]
    fn test_to_string_pretty() {
        let demo = Demo {
            name: "Hee".to_string(),
            age: 100,
        };

        let demo2 = Demo2 {
            nick: "222222".to_string(),
            level: 2,
        };

        let d1 = to_string_pretty(&demo).unwrap();
        let d2 = to_string_pretty(&demo2).unwrap();
        assert_eq!(
            r#"{
  "name": "Hee",
  "age": 100
}"#,
            d1.as_str()
        );

        let d3 = merge_json_string(d1, d2).unwrap();
        println!("{}", d3);
        assert_eq!(
            r#"{"age":100,"level":2,"name":"Hee","nick":"222222"}"#,
            d3.as_str()
        );
    }
}
