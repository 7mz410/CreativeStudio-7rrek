//! Notes: pinned notes on the canvas that a person leaves for an agent, and the agent marks done.
//! `note.add` places one (undoable), `note.list` reads them, `note.done` marks one done and
//! `note.remove` deletes it. They are kept in the document's preserved data under `"notes"`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::Document;

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

const KEY: &str = "notes";
/// Most characters one note's text may hold.
const MAX_TEXT: usize = 2000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "note.add",
            "Add Note",
            [],
            None,
            "{x: number, y: number (document points, finite), text: string (1–2000 characters, not blank)} adds a note pinned at x, y → {id}",
            has_doc,
            add
        ),
        cmd!(
            query "note.list",
            "Notes",
            [],
            None,
            "{} → {notes: [{id, x, y, text, done, by}]}; by is the actor that added the note (null for the person at the app)",
            has_doc,
            list
        ),
        cmd!(
            "note.done",
            "Mark Note Done",
            [],
            None,
            "{id: number, done?: bool (default true)} marks the note done (or, with done: false, not done again) → {id, done}",
            has_doc,
            done
        ),
        cmd!("note.remove", "Remove Note", [], None, "{id: number} deletes the note → {id}", has_doc, remove),
    ]
}

/// One note, as the document keeps it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Note {
    id: u64,
    x: f64,
    y: f64,
    text: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    by: Option<String>,
}

fn notes_of(doc: &Document) -> Vec<Note> {
    doc.unknown.get(KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

fn store(doc: &mut Document, notes: &[Note]) {
    if let Ok(v) = serde_json::to_value(notes) {
        doc.unknown.insert(KEY.into(), v);
    }
}

fn notes_json(notes: &[Note]) -> Value {
    Value::Array(notes.iter().filter_map(|n| serde_json::to_value(n).ok()).collect())
}

/// The note id from `p[key]`: a whole number.
fn id_of(p: &Value, key: &str, c: &str) -> Result<u64> {
    p.get(key).and_then(Value::as_u64).ok_or_else(|| bad(c, format!("{key} must be a whole number")))
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "note.add";
    let coord =
        |key: &str| p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad(C, format!("{key} must be a finite number")));
    let (x, y) = (coord("x")?, coord("y")?);
    let text = str_param(p, "text").map(str::trim).unwrap_or_default();
    if text.is_empty() {
        return Err(bad(C, "text is required"));
    }
    if text.chars().count() > MAX_TEXT {
        return Err(bad(C, format!("text is longer than {MAX_TEXT} characters")));
    }
    let text = text.to_string();
    let by = s.actor.clone();
    let id = s.edit("Add Note", |d, _| {
        let mut notes = notes_of(d);
        let id = notes.iter().map(|n| n.id).max().map_or(1, |m| m.saturating_add(1));
        notes.push(Note { id, x, y, text: text.clone(), done: false, by: by.clone() });
        store(d, &notes);
        Ok(id)
    })?;
    Ok(json!({"id": id}))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let notes = notes_of(&s.doc()?.doc);
    Ok(json!({"notes": notes_json(&notes)}))
}

fn done(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "note.done";
    let id = id_of(p, "id", C)?;
    let done = match p.get("done") {
        None | Some(Value::Null) => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(C, "done must be true or false"))?,
    };
    s.edit("Mark Note Done", |d, _| {
        let mut notes = notes_of(d);
        let n = notes.iter_mut().find(|n| n.id == id).ok_or_else(|| bad(C, format!("no note with id {id}")))?;
        n.done = done;
        store(d, &notes);
        Ok(())
    })?;
    Ok(json!({"id": id, "done": done}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "note.remove";
    let id = id_of(p, "id", C)?;
    s.edit("Remove Note", |d, _| {
        let mut notes = notes_of(d);
        let before = notes.len();
        notes.retain(|n| n.id != id);
        if notes.len() == before {
            return Err(bad(C, format!("no note with id {id}")));
        }
        store(d, &notes);
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s
    }

    #[test]
    fn add_list_done_remove() {
        let mut s = session();
        let a = s.execute("note.add", &json!({"x": 10, "y": 20.5, "text": "  make it blue  "})).unwrap();
        assert_eq!(a["id"], 1);
        let b = s.execute("note.add", &json!({"x": 0, "y": 0, "text": "second"})).unwrap();
        assert_eq!(b["id"], 2);
        let notes = s.execute("note.list", &json!({})).unwrap()["notes"].clone();
        assert_eq!(notes[0], json!({"id": 1, "x": 10.0, "y": 20.5, "text": "make it blue", "done": false, "by": null}));
        assert_eq!(s.execute("note.done", &json!({"id": 1})).unwrap(), json!({"id": 1, "done": true}));
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"][0]["done"], true);
        s.execute("note.done", &json!({"id": 1, "done": false})).unwrap();
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"][0]["done"], false);
        s.execute("note.remove", &json!({"id": 2})).unwrap();
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn add_is_undoable_and_records_actor() {
        let mut s = session();
        s.actor = Some("agent".into());
        s.execute("note.add", &json!({"x": 1, "y": 2, "text": "from agent"})).unwrap();
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"][0]["by"], "agent");
        let st = s.active().unwrap();
        assert_eq!(st.history.undo.last().unwrap().by.as_deref(), Some("agent"));
        assert_eq!(st.history.undo.last().unwrap().label, "Add Note");
        s.actor = None;
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"], json!([]));
        s.execute("note.add", &json!({"x": 1, "y": 2, "text": "by person"})).unwrap();
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"][0]["by"], Value::Null);
        assert_eq!(s.active().unwrap().history.undo.last().unwrap().by, None);
    }

    #[test]
    fn bad_params_error() {
        let mut s = session();
        for p in [
            json!({"x": 1, "y": 2}),
            json!({"x": 1, "y": 2, "text": "   "}),
            json!({"x": "a", "y": 2, "text": "t"}),
            json!({"x": 1, "y": 2, "text": "x".repeat(MAX_TEXT + 1)}),
        ] {
            assert!(s.execute("note.add", &p).is_err(), "{p}");
        }
        assert!(s.execute("note.done", &json!({"id": 9})).is_err());
        assert!(s.execute("note.done", &json!({"id": 1})).is_err());
        assert!(s.execute("note.remove", &json!({"id": "1"})).is_err());
        assert_eq!(s.execute("note.list", &json!({})).unwrap()["notes"], json!([]));
    }
}
