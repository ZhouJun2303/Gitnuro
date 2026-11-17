use serde_json::Value;

/// Apply only edits relative to the writer's baseline. Unrelated window edits survive.
pub fn apply(current: &Value, before: &Value, edited: &Value) -> Value {
    if edited == before { return current.clone(); }
    if let (Some(current), Some(before), Some(edited)) = (current.as_object(), before.as_object(), edited.as_object()) {
        let mut result = current.clone();
        for (key, value) in edited { result.insert(key.clone(), apply(current.get(key).unwrap_or(&Value::Null), before.get(key).unwrap_or(&Value::Null), value)); }
        for key in before.keys() { if !edited.contains_key(key) { result.remove(key); } }
        return Value::Object(result);
    }
    if let (Some(current), Some(before), Some(edited)) = (current.as_array(), before.as_array(), edited.as_array()) {
        if current.iter().chain(before).chain(edited).all(|row| row.get("id").and_then(Value::as_str).is_some()) {
            let id = |row: &Value| row["id"].as_str().unwrap().to_string();
            let mut result = Vec::new();
            for row in current {
                let old = before.iter().find(|old| id(old) == id(row));
                let new = edited.iter().find(|new| id(new) == id(row));
                match (old, new) {
                    (Some(old), Some(new)) => result.push(apply(row, old, new)),
                    (Some(_), None) => {},
                    (None, Some(new)) => result.push(apply(row, &Value::Null, new)),
                    (None, None) => result.push(row.clone()),
                }
            }
            // A concurrent deletion stays deleted; a newly created row is appended.
            for row in edited { if !current.iter().any(|old| id(old) == id(row)) && !before.iter().any(|old| id(old) == id(row)) { result.push(row.clone()); } }
            return Value::Array(result);
        }
    }
    edited.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn account_settings_merge_keeps_other_window_edits_additions_and_deletions() {
        let before = json!({ "theme": "light", "accounts": [{"id":"a", "label":"old", "token":"old"}, {"id":"removed"}] });
        let current = json!({ "theme": "dark", "accounts": [{"id":"a", "label":"new", "token":"old"}, {"id":"other"}] });
        let edited = json!({ "theme": "light", "accounts": [{"id":"a", "label":"old", "token":"refreshed"}, {"id":"removed"}, {"id":"found"}] });
        assert_eq!(apply(&current, &before, &edited), json!({"theme":"dark", "accounts":[{"id":"a","label":"new","token":"refreshed"},{"id":"other"},{"id":"found"}]}));
    }
}
